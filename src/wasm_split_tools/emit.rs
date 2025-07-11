#![allow(clippy::needless_range_loop)]

use crate::internal_prelude::*;
use md5::{Digest, Md5};
use std::{
    collections::{HashMap, HashSet},
    convert::identity,
    ops::Range,
};

use super::{
    dep_graph::DepNode,
    read::{InputFuncId, InputModule},
    split_point::{OutputModuleInfo, SplitModuleIdentifier, SplitProgramInfo},
};
use base64ct::{Base64UrlUnpadded, Encoding};
use wasmparser::{DataKind, RelocationEntry, RelocationType, SymbolInfo};

fn is_indirect_function_reloc(ty: RelocationType) -> bool {
    use RelocationType::*;
    matches!(
        ty,
        TableIndexSleb
            | TableIndexI32
            | TableIndexRelSleb
            | TableIndexSleb64
            | TableIndexI64
            | TableIndexRelSleb64
    )
}

fn get_indirect_functions(module: &InputModule) -> Result<HashSet<InputFuncId>> {
    let mut funcs = HashSet::new();

    for relocs in [module.code_section_index, module.data_section_index]
        .iter()
        .filter_map(|section_index| module.relocs.get(section_index))
    {
        for entry in relocs.iter() {
            if is_indirect_function_reloc(entry.ty) {
                let symbol = &module.symbols[entry.index as usize];
                let SymbolInfo::Func { index, .. } = symbol else {
                    bail!("Invalid symbol {symbol:?} referenced by relocation {entry:?}");
                };
                funcs.insert(*index as usize);
            }
        }
    }

    Ok(funcs)
}

#[derive(Debug)]
struct EmitState {
    indirect_functions: IndirectFunctionEmitInfo,
    // All relocations, ordered by offset, which are relative to the start of
    // the file rather than the start of the section.
    all_relocations: Vec<RelocationEntry>,
}

impl EmitState {
    fn new(module: &InputModule, program_info: &SplitProgramInfo) -> Result<Self> {
        let indirect_functions = IndirectFunctionEmitInfo::new(module, program_info)?;
        let mut all_relocations = Vec::<RelocationEntry>::new();
        for (section_index, section_offset) in [
            (module.code_section_index, module.code_section_offset),
            (module.data_section_index, module.data_section_offset),
        ] {
            let Some(section_relocs) = module.relocs.get(&section_index) else {
                continue;
            };
            for reloc in section_relocs {
                let mut reloc = *reloc;
                reloc.offset =
                    reloc
                        .offset
                        .checked_add(section_offset as u32)
                        .ok_or_else(|| {
                            anyhow!(
                                "Invalid relocation {reloc:?} for section offset {section_offset:?}"
                            )
                        })?;
                all_relocations.push(reloc);
            }
        }
        all_relocations.sort_by_key(|reloc| reloc.offset);

        Ok(EmitState {
            indirect_functions,
            all_relocations,
        })
    }

    fn get_relocations_for_range(&self, range: &Range<usize>) -> &[RelocationEntry] {
        let start = self
            .all_relocations
            .binary_search_by_key(&range.start, |reloc| reloc.offset as usize)
            .map_or_else(identity, identity);
        let end = self
            .all_relocations
            .binary_search_by_key(&range.end, |reloc| reloc.offset as usize)
            .map_or_else(identity, identity);
        &self.all_relocations[start..end]
    }
}

#[derive(Debug, Default)]
struct IndirectFunctionEmitInfo {
    table_entries: Vec<InputFuncId>,
    function_table_index: HashMap<InputFuncId, usize>,
    table_range_for_output_module: Vec<Range<usize>>,
}

impl IndirectFunctionEmitInfo {
    fn new(module: &InputModule, program_info: &SplitProgramInfo) -> Result<Self> {
        let mut indirect_functions = get_indirect_functions(module)?;

        indirect_functions.extend(program_info.shared_funcs.iter());

        // Remove all split point imports. These are placeholders. Any
        // references to these functions will be replaced by a reference to the
        // corresponding `SplitPoint::export_func`.
        for (_, output_module) in program_info.output_modules.iter() {
            for split_point in output_module.split_points.iter() {
                indirect_functions.remove(&split_point.import_func);
            }
        }

        let mut table_entries: Vec<_> = indirect_functions.into_iter().collect();
        table_entries.sort_by_key(|&func_id| {
            (
                program_info
                    .symbol_output_module
                    .get(&DepNode::Function(func_id)),
                func_id,
            )
        });
        let function_table_index: HashMap<_, _> = table_entries
            .iter()
            .enumerate()
            .map(|(i, func_id)| (*func_id, i + 1))
            .collect();

        let mut table_range_for_output_module: Vec<Range<usize>> = program_info
            .output_modules
            .iter()
            .map(|_| Range {
                start: usize::MAX,
                end: 0,
            })
            .collect();

        for (&func, &table_index) in function_table_index.iter() {
            if let Some(&output_module_index) = program_info
                .symbol_output_module
                .get(&DepNode::Function(func))
            {
                let range = &mut table_range_for_output_module[output_module_index];
                range.start = range.start.min(table_index);
                range.end = range.end.max(table_index + 1);
            }
        }

        Ok(Self {
            table_entries,
            function_table_index,
            table_range_for_output_module,
        })
    }
}

fn encode_leb128_u32_5byte(mut value: u32, buf: &mut [u8; 5]) {
    for i in 0..5 {
        buf[i] = (value as u8) & 0x7f;
        value >>= 7;
    }
    for i in 0..4 {
        buf[i] |= 0x80;
    }
}

fn encode_leb128_i32_5byte(mut value: i32, buf: &mut [u8; 5]) {
    for i in 0..5 {
        buf[i] = (value as u8) & 0x7f;
        value >>= 7;
    }
    for i in 0..4 {
        buf[i] |= 0x80;
    }
}

fn encode_leb128_i64_10byte(mut value: i64, buf: &mut [u8; 10]) {
    for i in 0..10 {
        buf[i] = (value as u8) & 0x7f;
        value >>= 7;
    }
    for i in 0..9 {
        buf[i] |= 0x80;
    }
}

fn encode_u32(value: u32, buf: &mut [u8; 4]) {
    *buf = value.to_le_bytes();
}

fn encode_u64(value: u64, buf: &mut [u8; 8]) {
    *buf = value.to_le_bytes();
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Copy)]
enum OutputFunctionKind {
    Import,
    Defined,
    IndirectStub,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct OutputFunction {
    kind: OutputFunctionKind,
    input_func_id: InputFuncId,
}

struct ModuleEmitState<'a> {
    input_module: &'a InputModule<'a>,
    output_module_index: usize,
    output_module_info: &'a OutputModuleInfo,
    emit_state: &'a EmitState,
    output_module: wasm_encoder::Module,
    output_functions: Vec<OutputFunction>,
    input_function_output_id: HashMap<InputFuncId, usize>,
    indirect_function_table_range: Range<usize>,
}

impl<'a> ModuleEmitState<'a> {
    fn new(
        module: &'a InputModule<'a>,
        emit_state: &'a EmitState,
        output_module_index: usize,
        program_info: &'a super::split_point::SplitProgramInfo,
    ) -> Self {
        let output_module_info = &program_info.output_modules[output_module_index].1;
        // We need to include definitions for all of the `included_symbols`.
        let mut funcs_to_define = HashSet::<InputFuncId>::new();
        funcs_to_define.extend(output_module_info.included_symbols.iter().filter_map(
            |dep| match dep {
                DepNode::Function(func_id) => Some(func_id),
                _ => None,
            },
        ));
        // In addition, we need to include a stub function for each shared
        // import that forwards to an indirect call. These stub functions allow
        // us to replace calls to functions defined in other modules using the
        // relocation entries alone. These stubs will potentially be optimized
        // out by `wasm-opt`.
        funcs_to_define.extend(output_module_info.shared_imports.iter());

        let mut output_functions: Vec<_> = funcs_to_define
            .iter()
            .map(|&func_id| {
                let kind = if output_module_info
                    .included_symbols
                    .contains(&DepNode::Function(func_id))
                {
                    if func_id < module.imported_funcs.len() {
                        OutputFunctionKind::Import
                    } else {
                        OutputFunctionKind::Defined
                    }
                } else {
                    OutputFunctionKind::IndirectStub
                };
                OutputFunction {
                    kind,
                    input_func_id: func_id,
                }
            })
            .collect();

        output_functions.sort();

        let mut input_function_output_id: HashMap<_, _> = output_functions
            .iter()
            .enumerate()
            .map(|(output_func_id, &OutputFunction { input_func_id, .. })| {
                (input_func_id, output_func_id)
            })
            .collect();

        // Map references to `import_func` to `export_func`.
        for (_, output_module) in program_info.output_modules.iter() {
            for split_point in output_module.split_points.iter() {
                if let Some(&output_func_id) =
                    input_function_output_id.get(&split_point.export_func)
                {
                    trace!("Mapping split point {split_point:?} -> {output_func_id}");
                    input_function_output_id.insert(split_point.import_func, output_func_id);
                }
            }
        }

        let indirect_function_table_range =
            emit_state.indirect_functions.table_range_for_output_module[output_module_index]
                .clone();

        Self {
            input_module: module,
            output_module_index,
            output_module_info,
            emit_state,
            output_module: wasm_encoder::Module::new(),
            output_functions,
            input_function_output_id,
            indirect_function_table_range,
        }
    }

    fn is_main(&self) -> bool {
        self.output_module_index == 0
    }

    fn get_relocation_input_function_index(&self, relocation: &RelocationEntry) -> Result<usize> {
        let Some(SymbolInfo::Func {
            index: input_func_id,
            ..
        }) = self.input_module.symbols.get(relocation.index as usize)
        else {
            bail!("Relocation {relocation:?} does not refer to a valid function");
        };
        Ok(*input_func_id as usize)
    }

    fn get_relocated_function_index(&self, relocation: &RelocationEntry) -> Result<usize> {
        let input_func_id = self.get_relocation_input_function_index(relocation)?;
        let Some(&output_func_id) = self.input_function_output_id.get(&input_func_id) else {
            bail!(
                "Dependency analysis error: \
                 No output function for input function {input_func_id} \
                 referenced by relocation {relocation:?}"
            );
        };
        Ok(output_func_id)
    }

    fn get_relocated_function_table_index(&self, relocation: &RelocationEntry) -> Result<usize> {
        let input_func_id = self.get_relocation_input_function_index(relocation)?;
        self.emit_state
            .indirect_functions
            .function_table_index
            .get(&input_func_id)
            .ok_or_else(|| {
                anyhow!(
                    "Dependency analysis error: \
                     No indirect function table index \
                     for input function {input_func_id} \
                     referenced by relocation {relocation:?}"
                )
            })
            .copied()
    }

    fn apply_relocation(
        &self,
        data: &mut [u8],
        data_offset: usize,
        relocation: &RelocationEntry,
    ) -> Result<()> {
        let relocation_range = relocation.relocation_range();
        let target =
            &mut data[(relocation_range.start - data_offset)..(relocation_range.end - data_offset)];
        use RelocationType::*;
        match relocation.ty {
            FunctionIndexLeb => {
                encode_leb128_u32_5byte(
                    self.get_relocated_function_index(relocation)? as u32,
                    target.try_into().unwrap(),
                );
            }
            TableIndexSleb => {
                encode_leb128_i32_5byte(
                    self.get_relocated_function_table_index(relocation)? as i32,
                    target.try_into().unwrap(),
                );
            }
            TableIndexI32 => {
                encode_u32(
                    self.get_relocated_function_table_index(relocation)? as u32,
                    target.try_into().unwrap(),
                );
            }
            TableIndexSleb64 => {
                encode_leb128_i64_10byte(
                    self.get_relocated_function_table_index(relocation)? as i64,
                    target.try_into().unwrap(),
                );
            }
            TableIndexI64 => {
                encode_u64(
                    self.get_relocated_function_table_index(relocation)? as u64,
                    target.try_into().unwrap(),
                );
            }
            FunctionIndexI32 => {
                encode_u32(
                    self.get_relocated_function_index(relocation)? as u32,
                    target.try_into().unwrap(),
                );
            }
            FunctionOffsetI32 | SectionOffsetI32 | TableIndexRelSleb | FunctionOffsetI64
            | TableIndexRelSleb64 => {
                bail!("Unsupported relocation type {relocation:?}");
            }
            _ => {}
        }
        Ok(())
    }

    fn get_relocated_data(&self, range: Range<usize>) -> Result<Vec<u8>> {
        let mut data = Vec::from(&self.input_module.raw[range.clone()]);
        for relocation in self.emit_state.get_relocations_for_range(&range) {
            self.apply_relocation(&mut data, range.start, relocation)?;
        }
        Ok(data)
    }

    fn generate(&mut self) -> Result<()> {
        // Encode type section
        self.generate_type_section()?;
        self.generate_import_section();
        self.generate_function_section()?;
        self.generate_table_section();
        self.generate_memory_section();
        self.generate_global_section();
        self.generate_export_section();
        self.generate_start_section();
        self.generate_element_section()?;
        self.generate_data_count_section();
        self.generate_code_section()?;
        self.generate_data_section()?;
        self.generate_wasm_bindgen_sections();
        self.generate_name_section()?;
        self.generate_target_features_section();
        Ok(())
    }

    fn generate_type_section(&mut self) -> Result<()> {
        // Simply copy all types.  Unneeded types may be pruned by `wasm-opt`.
        let mut section = wasm_encoder::TypeSection::new();
        for input_func_type in self.input_module.types.iter() {
            let output_func_type: wasm_encoder::FuncType =
                input_func_type.clone().try_into().unwrap();
            section.function(
                output_func_type.params().iter().cloned(),
                output_func_type.results().iter().cloned(),
            );
        }
        self.output_module.section(&section);
        Ok(())
    }

    fn get_global_name(&self, index: usize) -> String {
        self.input_module
            .names
            .globals
            .get(&index)
            .map(|name| name.to_string())
            .or_else(|| {
                self.input_module
                    .export_map
                    .get(&(wasmparser::ExternalKind::Global as isize, index))
                    .map(|(_, name)| name.to_string())
            })
            .unwrap_or_else(|| format!("__global_{index}"))
    }

    fn get_memory_name(&self, index: usize) -> String {
        self.input_module
            .names
            .memories
            .get(&index)
            .map(|name| name.to_string())
            .or_else(|| {
                self.input_module
                    .export_map
                    .get(&(wasmparser::ExternalKind::Memory as isize, index))
                    .map(|(_, name)| name.to_string())
            })
            .unwrap_or_else(|| format!("__memory_{index}"))
    }

    fn get_indirect_function_table_type(&self) -> wasm_encoder::TableType {
        // + 1 due to empty entry at index 0
        let indirect_table_size = self.emit_state.indirect_functions.table_entries.len() + 1;
        wasm_encoder::TableType {
            element_type: wasm_encoder::RefType::FUNCREF,
            minimum: indirect_table_size as u32,
            maximum: Some(indirect_table_size as u32),
        }
    }

    fn generate_import_section(&mut self) {
        let mut section = wasm_encoder::ImportSection::new();
        // Function imports
        for (func_id, &import_id) in self.input_module.imported_funcs.iter().enumerate() {
            if !self
                .output_module_info
                .included_symbols
                .contains(&DepNode::Function(func_id))
            {
                continue;
            }
            let import = &self.input_module.imports[import_id];
            let ty: wasm_encoder::EntityType = import.ty.try_into().unwrap();
            section.import(import.module, import.name, ty);
        }

        // Copy all non-function imports from input.
        for import in self.input_module.imports.iter() {
            if let wasmparser::TypeRef::Func(_) = import.ty {
                continue;
            }
            let ty: wasm_encoder::EntityType = import.ty.try_into().unwrap();
            section.import(import.module, import.name, ty);
        }

        if !self.is_main() {
            // Import indirect function table.

            section.import(
                "__wasm_split",
                "__indirect_function_table",
                self.get_indirect_function_table_type(),
            );

            // Import all globals defined by the input module.
            for (global_index, global) in self.input_module.globals.iter().enumerate() {
                let ty: wasm_encoder::GlobalType = global.ty.try_into().unwrap();
                if !ty.mutable {
                    continue;
                }
                section.import(
                    "__wasm_split",
                    self.get_global_name(global_index).as_str(),
                    ty,
                );
            }

            // Import all memories defined by the input module.
            for (memory_index, memory) in self.input_module.memories.iter().enumerate() {
                let ty: wasm_encoder::MemoryType = (*memory).into();
                section.import(
                    "__wasm_split",
                    self.get_memory_name(memory_index).as_str(),
                    ty,
                );
            }
        }
        self.output_module.section(&section);
    }

    fn generate_function_section(&mut self) -> Result<()> {
        let mut section = wasm_encoder::FunctionSection::new();
        for OutputFunction { input_func_id, .. } in self
            .output_functions
            .iter()
            .filter(|OutputFunction { kind, .. }| *kind != OutputFunctionKind::Import)
        {
            section.function(self.input_module.func_type_id(*input_func_id)? as u32);
        }
        self.output_module.section(&section);
        Ok(())
    }

    fn generate_table_section(&mut self) {
        if !self.is_main() {
            return;
        }
        let mut section = wasm_encoder::TableSection::new();
        section.table(self.get_indirect_function_table_type());
        self.output_module.section(&section);
    }

    fn generate_memory_section(&mut self) {
        if !self.is_main() || self.input_module.memories.is_empty() {
            return;
        }
        let mut section = wasm_encoder::MemorySection::new();
        for memory in self.input_module.memories.iter() {
            section.memory((*memory).into());
        }
        self.output_module.section(&section);
    }

    fn generate_global_section(&mut self) {
        if !self.is_main() {
            return;
        }
        let mut section = wasm_encoder::GlobalSection::new();
        for global in self.input_module.globals.iter() {
            section.global(
                global.ty.try_into().unwrap(),
                &global.init_expr.try_into().unwrap(),
            );
        }
        self.output_module.section(&section);
    }

    fn generate_export_section(&mut self) {
        if !self.is_main() {
            return;
        }
        let mut section = wasm_encoder::ExportSection::new();
        let mut existing_exports = HashSet::<&str>::new();
        for export in self.input_module.exports.iter() {
            let mut index = export.index;
            if export.kind == wasmparser::ExternalKind::Func {
                let Some(&func_id) = self.input_function_output_id.get(&(index as InputFuncId))
                else {
                    continue;
                };
                index = func_id as u32;
            }
            section.export(export.name, export.kind.into(), index);
            existing_exports.insert(export.name);
        }

        // Export table.
        if !existing_exports.contains("__indirect_function_table") {
            section.export(
                "__indirect_function_table",
                wasm_encoder::ExportKind::Table,
                0,
            );
        }

        // Export globals.
        for (global_index, global) in self.input_module.globals.iter().enumerate() {
            let name = self.get_global_name(global_index);
            if existing_exports.contains(name.as_str()) {
                continue;
            }
            if !global.ty.mutable {
                break;
            }
            section.export(
                name.as_str(),
                wasm_encoder::ExportKind::Global,
                global_index as u32,
            );
        }
        self.output_module.section(&section);
    }

    fn generate_start_section(&mut self) {
        if self.is_main() {
            if let Some(input_start_func_id) = self.input_module.start {
                let output_func = self
                    .input_function_output_id
                    .get(&input_start_func_id)
                    .expect("Failed to map start function to output function index");
                self.output_module.section(&wasm_encoder::StartSection {
                    function_index: *output_func as u32,
                });
            }
        }
    }

    fn generate_element_section(&mut self) -> Result<()> {
        let indirect_range = self.indirect_function_table_range.clone();
        if indirect_range.is_empty() {
            //panic!("No indirect range");
            return Ok(());
        }
        let mut section = wasm_encoder::ElementSection::new();
        let func_ids: Vec<u32> = indirect_range
            .clone()
            .map(|table_index| -> Result<u32> {
                let input_func_id =
                    self.emit_state.indirect_functions.table_entries[table_index - 1];
                let output_func_id = *self
                    .input_function_output_id
                    .get(&input_func_id)
                    .ok_or_else(|| {
                        anyhow!(
                            "No output function corresponding to input function {input_func_id:?}"
                        )
                    })?;
                Ok(output_func_id as u32)
            })
            .collect::<Result<Vec<_>>>()?;
        section.segment(wasm_encoder::ElementSegment {
            mode: wasm_encoder::ElementMode::Active {
                table: Some(0),
                offset: &wasm_encoder::ConstExpr::i32_const(indirect_range.start as i32),
            },
            elements: wasm_encoder::Elements::Functions(&func_ids),
        });
        self.output_module.section(&section);
        Ok(())
    }

    fn generate_data_count_section(&mut self) {
        let section = wasm_encoder::DataCountSection {
            count: if self.is_main() {
                self.input_module.data_segments.len() as u32
            } else {
                0
            },
        };
        self.output_module.section(&section);
    }

    fn generate_indirect_stub(
        &self,
        indirect_index: usize,
        type_id: usize,
    ) -> wasm_encoder::Function {
        let func_type = &self.input_module.types[type_id];
        let mut func = wasm_encoder::Function::new([]);
        for (param_i, _param_type) in func_type.params().iter().enumerate() {
            func.instruction(&wasm_encoder::Instruction::LocalGet(param_i as u32));
        }
        func.instruction(&wasm_encoder::Instruction::I32Const(indirect_index as i32));
        func.instruction(&wasm_encoder::Instruction::CallIndirect {
            ty: type_id as u32,
            table: 0,
        });
        func.instruction(&wasm_encoder::Instruction::End);
        func
    }

    fn generate_code_section(&mut self) -> Result<()> {
        let mut section = wasm_encoder::CodeSection::new();
        for output_func in self.output_functions.iter() {
            match output_func.kind {
                OutputFunctionKind::Import => {}
                OutputFunctionKind::Defined => {
                    let input_func = &self.input_module.defined_funcs
                        [output_func.input_func_id - self.input_module.imported_funcs.len()];
                    section.raw(&self.get_relocated_data(input_func.body.range())?);
                }
                OutputFunctionKind::IndirectStub => {
                    let indirect_index = self
                        .emit_state
                        .indirect_functions
                        .function_table_index
                        .get(&output_func.input_func_id)
                        .unwrap();
                    let function = self.generate_indirect_stub(
                        *indirect_index,
                        self.input_module.func_type_id(output_func.input_func_id)?,
                    );
                    section.function(&function);
                }
            }
        }
        self.output_module.section(&section);
        Ok(())
    }

    fn generate_data_section(&mut self) -> Result<()> {
        if !self.is_main() {
            return Ok(());
        }
        let mut section = wasm_encoder::DataSection::new();
        for input_segment in self.input_module.data_segments.iter() {
            // Note: `input_segment.range` includes the segment header.
            let range_end = input_segment.range.end;
            let data =
                self.get_relocated_data((range_end - input_segment.data.len())..range_end)?;
            match input_segment.kind {
                DataKind::Passive => section.passive(data),
                DataKind::Active {
                    memory_index,
                    offset_expr,
                } => section.active(memory_index, &offset_expr.try_into()?, data),
            };
        }
        self.output_module.section(&section);
        Ok(())
    }

    fn generate_name_section(&mut self) -> Result<()> {
        fn convert_name_map<'a>(
            parser_map: &wasmparser::NameMap<'a>,
        ) -> Result<wasm_encoder::NameMap> {
            let mut encoder_map = wasm_encoder::NameMap::new();
            for r in parser_map.clone().into_iter() {
                let naming = r?;
                encoder_map.append(naming.index, naming.name);
            }
            Ok(encoder_map)
        }

        fn convert_name_hash_map(map: &HashMap<usize, &str>) -> wasm_encoder::NameMap {
            let mut encoder_map = wasm_encoder::NameMap::new();
            let mut names = map.iter().collect::<Vec<_>>();
            names.sort();
            for (i, name) in names.iter() {
                encoder_map.append(**i as u32, name);
            }
            encoder_map
        }
        let mut section = wasm_encoder::NameSection::new();
        // Function names
        {
            let mut name_map = wasm_encoder::NameMap::new();
            let mut locals_map = wasm_encoder::IndirectNameMap::new();
            let mut labels_map = wasm_encoder::IndirectNameMap::new();
            for (output_func_id, OutputFunction { input_func_id, .. }) in
                self.output_functions.iter().enumerate()
            {
                if let Some(name) = self.input_module.names.functions.get(input_func_id) {
                    name_map.append(output_func_id as u32, name);
                }
                if let Some(name_map) = self.input_module.names.locals.get(input_func_id) {
                    locals_map.append(output_func_id as u32, &convert_name_map(name_map)?);
                }
                if let Some(name_map) = self.input_module.names.labels.get(input_func_id) {
                    labels_map.append(output_func_id as u32, &convert_name_map(name_map)?);
                }
            }
            section.functions(&name_map);
            section.locals(&locals_map);
            section.labels(&labels_map);
        }
        section.types(&convert_name_hash_map(&self.input_module.names.types));
        section.tables(&convert_name_hash_map(&self.input_module.names.tables));
        section.memories(&convert_name_hash_map(&self.input_module.names.memories));
        section.globals(&convert_name_hash_map(&self.input_module.names.globals));
        // elements
        if self.is_main() {
            section.data(&convert_name_hash_map(
                &self.input_module.names.data_segments,
            ));
        }
        // tag
        // fields
        // tags
        self.output_module.section(&section);
        Ok(())
        // Type names
    }

    fn generate_wasm_bindgen_sections(&mut self) {
        for custom in self.input_module.custom_sections.iter() {
            if self.is_main() && custom.name == "__wasm_bindgen_unstable" {
                self.output_module.section(&wasm_encoder::CustomSection {
                    name: custom.name.into(),
                    data: custom.data.into(),
                });
            }
        }
    }

    fn generate_target_features_section(&mut self) {
        for custom in self.input_module.custom_sections.iter() {
            if custom.name == "target_features" {
                self.output_module.section(&wasm_encoder::CustomSection {
                    name: custom.name.into(),
                    data: custom.data.into(),
                });
            }
        }
    }
}

pub fn emit_modules(
    module: &InputModule,
    program_info: &mut SplitProgramInfo,
    // returns the hash of the contents
    mut emit_fn: impl FnMut(&SplitModuleIdentifier, &[u8], &str) -> Result<()>,
) -> Result<()> {
    // For now we will ignore data symbols because that simplifies things quite a bit.

    let emit_state = EmitState::new(module, program_info)?;

    for output_module_index in 0..program_info.output_modules.len() {
        let mut emit_state =
            ModuleEmitState::new(module, &emit_state, output_module_index, program_info);
        let identifier = &program_info.output_modules[output_module_index].0;

        emit_state
            .generate()
            .wrap_err(format!("Error generating {identifier:?}"))?;

        let data = emit_state.output_module.as_slice();
        let hash = Base64UrlUnpadded::encode_string(&Md5::new().chain_update(data).finalize());
        emit_fn(identifier, data, &hash).wrap_err(format!("Error emitting {identifier:?}"))?;

        let identifier = &mut program_info.output_modules[output_module_index].0;
        identifier.set_hash(hash);
    }

    Ok(())
}
