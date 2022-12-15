use std::{collections::HashSet, fmt::Display};

use derive_more::Display;
use itertools::Itertools;

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Outcome {
    Success(Product),
    Stopped,
}

#[derive(Debug, Display, Clone, PartialEq, Eq, Hash)]
pub enum Product {
    ServerBin,
    ClientWasm,
    Style,
    Assets,
    NoChange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductSet(HashSet<Product>);

impl ProductSet {
    pub fn from(vec: Vec<Outcome>) -> Self {
        Self(HashSet::from_iter(vec.into_iter().filter_map(
            |entry| match entry {
                Outcome::Success(Product::NoChange) => None,
                Outcome::Success(v) => Some(v),
                _ => None,
            },
        )))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn only_style(&self) -> bool {
        self.0.contains(&Product::Style) && self.0.len() == 1
    }

    pub fn contains(&self, product: &Product) -> bool {
        self.0.contains(product)
    }
}

impl Display for ProductSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.0
                .iter()
                .map(|f| f.to_string())
                .collect_vec()
                .join(", ")
        )
    }
}
