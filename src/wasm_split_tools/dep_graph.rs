use super::read::{InputFuncId, InputModule, SymbolIndex};
use crate::internal_prelude::*;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fmt::Debug,
    ops::Range,
};

#[derive(Debug, PartialEq, Eq, Hash, Copy, PartialOrd, Ord, Clone)]
pub enum DepNode {
    Function(InputFuncId),
    DataSymbol(SymbolIndex),
}

pub type DepGraph = HashMap<DepNode, HashSet<DepNode>>;

pub trait SymbolTable {
    fn get_symbol_dep_node(&self, symbol_index: SymbolIndex) -> Option<DepNode>;
}

impl<'a> SymbolTable for InputModule<'a> {
    fn get_symbol_dep_node(&self, symbol_index: SymbolIndex) -> Option<DepNode> {
        match self.symbols[symbol_index] {
            wasmparser::SymbolInfo::Func { index, .. } => {
                Some(DepNode::Function(index as InputFuncId))
            }
            wasmparser::SymbolInfo::Data { .. } => Some(DepNode::DataSymbol(symbol_index)),
            _ => None,
        }
    }
}

pub fn get_dependencies(module: &InputModule) -> Result<DepGraph> {
    let mut deps = DepGraph::new();
    let mut add_dep = |a: DepNode, b: u32| {
        if let Some(target) = module.get_symbol_dep_node(b as usize) {
            deps.entry(a).or_default().insert(target);
        };
    };

    let shift_range =
        |range: Range<usize>, offset: usize| (range.start + offset)..(range.end + offset);

    if let Some(relocs) = module.relocs.get(&module.code_section_index) {
        for entry in relocs {
            let func_index = find_function_containing_range(
                module,
                shift_range(entry.relocation_range(), module.code_section_offset),
            )
            .wrap_err(format!("Invalid relocation entry {entry:?}"))?;
            add_dep(DepNode::Function(func_index), entry.index);
        }
    }

    if let Some(relocs) = module.relocs.get(&module.data_section_index) {
        for entry in relocs {
            let symbol_index = find_data_symbol_containing_range(
                module,
                shift_range(entry.relocation_range(), module.data_section_offset),
            )
            .wrap_err(format!("Invalid relocation entry {entry:?}"))?;
            add_dep(DepNode::DataSymbol(symbol_index), entry.index);
        }
    }
    Ok(deps)
}

fn find_function_containing_range(
    module: &super::read::InputModule,
    range: Range<usize>,
) -> Result<usize> {
    let func_index = find_by_range(&module.defined_funcs, &range, |defined_func| {
        defined_func.body.range()
    })
    .wrap_err(format!("No match for function relocation range {range:?}"))?;
    Ok(module.imported_funcs.len() + func_index)
}

fn find_data_symbol_containing_range(
    module: &super::read::InputModule,
    range: Range<usize>,
) -> Result<usize> {
    let index = find_by_range(&module.data_symbols, &range, |data_symbol| {
        data_symbol.range.clone()
    })
    .wrap_err(format!("No match for data relocation range {range:?}"))?;
    Ok(module.data_symbols[index].symbol_index)
}

fn find_by_range<T: Debug, U: Debug + Ord, F: Fn(&T) -> Range<U>>(
    items: &[T],
    range: &Range<U>,
    get_range: F,
) -> Result<usize> {
    let index = items
        .binary_search_by(|item| {
            let item_range = get_range(item);
            if item_range.end <= range.start {
                Ordering::Less
            } else if item_range.start <= range.start {
                Ordering::Equal
            } else {
                Ordering::Greater
            }
        })
        .or_else(|index| {
            bail!(
                "Prev range is: {:?}, next range is: {:?}",
                items.get(index - 1).map(|item| (item, get_range(item))),
                items.get(index).map(|item| (item, get_range(item)))
            )
        })?;
    if range.end > get_range(&items[index]).end {
        bail!(
            "Item {:?} has incompatible range {:?}",
            items[index],
            get_range(&items[index])
        )
    }
    Ok(index)
}
