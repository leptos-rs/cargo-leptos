use derive_more::Display;
use itertools::Itertools;
use std::{collections::HashSet, fmt};
use tokio::sync::broadcast;

lazy_static::lazy_static! {
  static ref PRODUCT_CHANGE_CHANNEL: broadcast::Sender::<ProductSet> = broadcast::channel::<ProductSet>(1).0;
}

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
    pub fn empty() -> Self {
        Self(HashSet::new())
    }

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

impl fmt::Display for ProductSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
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

pub struct ProductChange {}

impl ProductChange {
    pub fn subscribe() -> broadcast::Receiver<ProductSet> {
        PRODUCT_CHANGE_CHANNEL.subscribe()
    }

    pub fn send(set: ProductSet) {
        if let Err(e) = PRODUCT_CHANGE_CHANNEL.send(set) {
            log::error!("Error could not send product changes due to {e}")
        }
    }
}
