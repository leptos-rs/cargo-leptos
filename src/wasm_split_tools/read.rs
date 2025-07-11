use crate::internal_prelude::*;
use std::collections::HashMap;
pub use std::ops::Range;
pub use wasmparser::{
    Data, Element, Export, FuncType, FunctionBody, Global, Import, MemoryType, RelocationEntry,
    SymbolInfo, Table, TagType,
};
use wasmparser::{Payload, TypeRef};

pub struct CustomSection<'a> {
    pub name: &'a str,
    pub data_offset: usize,
    pub data: &'a [u8],
}

pub type FuncTypeId = usize;
pub type InputFuncId = usize;
pub type TableId = usize;
pub type ImportId = usize;
pub type ExportId = usize;
pub type MemoryId = usize;
pub type GlobalId = usize;
pub type ElementId = usize;
pub type DataSegmentId = usize;
pub type TagId = usize;
pub type SectionIndex = usize;

#[derive(Debug)]
pub struct DefinedFunc<'a> {
    pub type_id: FuncTypeId,
    pub body: FunctionBody<'a>,
}

#[derive(Default, Clone)]
pub struct Names<'a> {
    pub module: Option<&'a str>,
    pub functions: HashMap<InputFuncId, &'a str>,
    pub locals: HashMap<InputFuncId, wasmparser::NameMap<'a>>,
    pub labels: HashMap<InputFuncId, wasmparser::NameMap<'a>>,
    pub types: HashMap<FuncTypeId, &'a str>,
    pub tables: HashMap<TableId, &'a str>,
    pub memories: HashMap<MemoryId, &'a str>,
    pub globals: HashMap<GlobalId, &'a str>,
    pub elements: HashMap<ElementId, &'a str>,
    pub data_segments: HashMap<DataSegmentId, &'a str>,
    pub tags: HashMap<TagId, &'a str>,
}

fn convert_name_map<'a>(name_map: wasmparser::NameMap<'a>) -> Result<HashMap<usize, &'a str>> {
    name_map
        .into_iter()
        .map(|r| r.map(|naming| (naming.index as usize, naming.name)))
        .collect::<Result<HashMap<usize, &'a str>, _>>()
        .map_err(|e| e.into())
}

fn convert_indirect_name_map<'a>(
    indirect_name_map: wasmparser::IndirectNameMap<'a>,
) -> Result<HashMap<usize, wasmparser::NameMap<'a>>> {
    indirect_name_map
        .into_iter()
        .map(|r| -> Result<(usize, wasmparser::NameMap<'a>)> {
            let indirect_naming = r?;
            Ok((indirect_naming.index as usize, indirect_naming.names))
        })
        .collect::<Result<HashMap<_, _>, _>>()
}

impl<'a> Names<'a> {
    fn new(data: &'a [u8], original_offset: usize) -> Result<Self> {
        let mut names: Self = Default::default();
        for part in wasmparser::NameSectionReader::new(data, original_offset) {
            use wasmparser::Name;
            match part? {
                Name::Module { name, .. } => {
                    names.module = Some(name);
                }
                Name::Function(name_map) => {
                    names.functions = convert_name_map(name_map)?;
                }
                Name::Local(indirect_name_map) => {
                    names.locals = convert_indirect_name_map(indirect_name_map)?;
                }
                Name::Label(indirect_name_map) => {
                    names.labels = convert_indirect_name_map(indirect_name_map)?;
                }
                Name::Type(name_map) => {
                    names.types = convert_name_map(name_map)?;
                }
                Name::Table(name_map) => {
                    names.tables = convert_name_map(name_map)?;
                }
                Name::Memory(name_map) => {
                    names.memories = convert_name_map(name_map)?;
                }
                Name::Global(name_map) => {
                    names.globals = convert_name_map(name_map)?;
                }
                Name::Data(name_map) => {
                    names.data_segments = convert_name_map(name_map)?;
                }
                Name::Element(name_map) => {
                    names.elements = convert_name_map(name_map)?;
                }
                Name::Tag(name_map) => {
                    names.tags = convert_name_map(name_map)?;
                }
                Name::Field(_name_map) => {
                    bail!("Field names not supported");
                }
                Name::Unknown { ty, .. } => {
                    bail!("Unknown name subsection: {:?}", ty);
                }
            }
        }
        Ok(names)
    }
}

pub type SymbolIndex = usize;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DataSymbol {
    pub symbol_index: SymbolIndex,
    // Range relative to the start of the WebAssembly file.
    pub range: Range<usize>,
}

fn get_data_symbols(data_segments: &[Data], symbols: &[SymbolInfo]) -> Result<Vec<DataSymbol>> {
    let mut data_symbols = Vec::new();
    for (symbol_index, info) in symbols.iter().enumerate() {
        let SymbolInfo::Data {
            symbol: Some(symbol),
            ..
        } = info
        else {
            continue;
        };
        if symbol.size == 0 {
            // Ignore zero-size symbols since they cannot be the target of a relocation.
            continue;
        }
        let data_segment = data_segments
            .get(symbol.index as usize)
            .ok_or_else(|| anyhow!("Invalid data segment index in symbol: {:?}", symbol))?;
        if symbol
            .offset
            .checked_add(symbol.size)
            .ok_or_else(|| anyhow!("Invalid symbol: {symbol:?}"))? as usize
            > data_segment.data.len()
        {
            bail!(
                "Invalid symbol {symbol:?} for data segment of size {:?}",
                data_segment.data.len()
            );
        }
        let offset = data_segment.range.end - data_segment.data.len() + (symbol.offset as usize);
        let range = offset..(offset + symbol.size as usize);
        data_symbols.push(DataSymbol {
            symbol_index,
            range,
        });
    }
    data_symbols.sort_by_key(|symbol| symbol.range.start);
    Ok(data_symbols)
}

#[derive(Default)]
pub struct InputModule<'a> {
    pub raw: &'a [u8],
    pub types: Vec<FuncType>,
    pub imports: Vec<Import<'a>>,
    pub tables: Vec<Table<'a>>,
    pub tags: Vec<TagType>,
    pub globals: Vec<Global<'a>>,
    pub exports: Vec<Export<'a>>,
    pub export_map: HashMap<(isize, usize), (usize, &'a str)>,
    pub memories: Vec<MemoryType>,
    pub elements: Vec<Element<'a>>,
    pub code_section_offset: usize,
    pub code_section_index: usize,
    pub data_segments: Vec<Data<'a>>,
    pub data_section_offset: usize,
    pub data_section_index: usize,
    pub imported_funcs: Vec<ImportId>,
    pub imported_func_map: HashMap<ImportId, InputFuncId>,
    pub defined_funcs: Vec<DefinedFunc<'a>>,
    pub custom_sections: Vec<CustomSection<'a>>,
    pub start: Option<InputFuncId>,
    pub names: Names<'a>,
    pub symbols: Vec<SymbolInfo<'a>>,
    pub data_symbols: Vec<DataSymbol>,
    pub relocs: HashMap<usize, Vec<RelocationEntry>>,
}

impl<'a> InputModule<'a> {
    pub fn parse(wasm: &'a [u8]) -> Result<Self> {
        let mut module = Self {
            raw: wasm,
            ..Default::default()
        };
        let mut function_types: Vec<FuncTypeId> = Vec::new();
        let mut section_index = 0;
        let parser = wasmparser::Parser::new(0);
        for payload in parser.parse_all(wasm) {
            match payload? {
                Payload::Version { .. } => {}
                Payload::TypeSection(reader) => {
                    module.types = reader
                        .into_iter_err_on_gc_types()
                        .collect::<Result<Vec<_>, _>>()?;
                    section_index += 1;
                }
                Payload::ImportSection(reader) => {
                    module.imports = reader.into_iter().collect::<Result<Vec<_>, _>>()?;
                    section_index += 1;
                }
                Payload::FunctionSection(reader) => {
                    function_types = reader
                        .into_iter()
                        .map(|t| t.map(|id| id as FuncTypeId))
                        .collect::<Result<Vec<_>, _>>()?;
                    section_index += 1;
                }
                Payload::TableSection(reader) => {
                    module.tables = reader.into_iter().collect::<Result<Vec<_>, _>>()?;
                    section_index += 1;
                }
                Payload::MemorySection(reader) => {
                    module.memories = reader.into_iter().collect::<Result<Vec<_>, _>>()?;
                    section_index += 1;
                }
                Payload::TagSection(reader) => {
                    module.tags = reader.into_iter().collect::<Result<Vec<_>, _>>()?;
                    section_index += 1;
                }
                Payload::GlobalSection(reader) => {
                    module.globals = reader.into_iter().collect::<Result<Vec<_>, _>>()?;
                    section_index += 1;
                }
                Payload::ExportSection(reader) => {
                    module.exports = reader.into_iter().collect::<Result<Vec<_>, _>>()?;
                    module.export_map = module
                        .exports
                        .iter()
                        .enumerate()
                        .map(|(i, export)| {
                            (
                                (export.kind as isize, export.index as usize),
                                (i, export.name),
                            )
                        })
                        .collect();
                    section_index += 1;
                }
                Payload::StartSection { func, .. } => {
                    module.start = Some(func as usize);
                    section_index += 1;
                }
                Payload::ElementSection(reader) => {
                    module.elements = reader.into_iter().collect::<Result<Vec<_>, _>>()?;
                    section_index += 1;
                }
                Payload::DataCountSection { .. } => {
                    section_index += 1;
                }
                Payload::DataSection(reader) => {
                    module.data_section_offset = reader.range().start;
                    module.data_segments = reader.into_iter().collect::<Result<Vec<_>, _>>()?;
                    module.data_section_index = section_index;
                    section_index += 1;
                }
                Payload::CodeSectionStart { range, .. } => {
                    module.code_section_offset = range.start;
                    module.code_section_index = section_index;
                    section_index += 1;
                }
                Payload::CodeSectionEntry(body) => {
                    let index = module.defined_funcs.len();
                    module.defined_funcs.push(DefinedFunc {
                        type_id: function_types[index],
                        body,
                    });
                }
                Payload::CustomSection(reader) => {
                    module.custom_sections.push(CustomSection {
                        name: reader.name(),
                        data: reader.data(),
                        data_offset: reader.data_offset(),
                    });
                    section_index += 1;
                }
                Payload::End(_) => {}
                section => {
                    bail!("Unknown section: {:?}", section);
                }
            }
        }

        for section in module.custom_sections.iter() {
            if section.name == "name" {
                module.names = Names::new(section.data, section.data_offset)?;
            } else if section.name == "linking" {
                let reader =
                    wasmparser::LinkingSectionReader::new(section.data, section.data_offset)?;
                for subsection in reader.subsections() {
                    if let wasmparser::Linking::SymbolTable(map) = subsection? {
                        module.symbols = map.into_iter().collect::<Result<Vec<_>, _>>()?;
                    }
                }
            } else if section.name.starts_with("reloc.") {
                let reader =
                    wasmparser::RelocSectionReader::new(section.data, section.data_offset)?;
                module.relocs.insert(
                    reader.section_index() as SectionIndex,
                    reader
                        .entries()
                        .into_iter()
                        .collect::<Result<Vec<_>, _>>()?,
                );
            }
        }
        module.data_symbols = get_data_symbols(&module.data_segments, &module.symbols)?;
        module.imported_funcs = module
            .imports
            .iter()
            .enumerate()
            .filter_map(|(import_id, import)| match import.ty {
                TypeRef::Func(_) => Some(import_id as ImportId),
                _ => None,
            })
            .collect();
        module.imported_func_map = module
            .imported_funcs
            .iter()
            .enumerate()
            .map(|(func_id, &import_id)| (import_id, func_id))
            .collect();
        Ok(module)
    }

    pub fn func_type_id(&self, func_id: InputFuncId) -> Result<FuncTypeId> {
        if func_id < self.imported_funcs.len() {
            let import_id = self.imported_funcs[func_id];
            let wasmparser::TypeRef::Func(type_id) = self.imports[import_id].ty else {
                bail!("Expected import to be a function");
            };
            Ok(type_id as usize)
        } else {
            Ok(self.defined_funcs[func_id - self.imported_funcs.len()].type_id)
        }
    }
}
