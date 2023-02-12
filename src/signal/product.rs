use derive_more::Display;
use itertools::Itertools;
use std::{collections::HashSet, fmt};
use tokio::sync::broadcast;

lazy_static::lazy_static! {
  static ref SERVER_RESTART_CHANNEL: broadcast::Sender::<()> = broadcast::channel::<()>(1).0;
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Outcome {
    Success(Product),
    Stopped,
    Failed,
}

impl Outcome {
    pub fn is_success(&self) -> bool {
        matches!(self, Outcome::Success(_))
    }
}

#[derive(Debug, Display, Clone, PartialEq, Eq, Hash)]
pub enum Product {
    Server,
    Front,
    Style,
    Assets,
    None,
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
                Outcome::Success(Product::None) => None,
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

    pub fn contains_any(&self, of: &[Product]) -> bool {
        of.iter().any(|p| self.0.contains(p))
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

pub struct ServerRestart {}

impl ServerRestart {
    pub fn subscribe() -> broadcast::Receiver<()> {
        SERVER_RESTART_CHANNEL.subscribe()
    }

    pub fn send() {
        log::trace!("Server restart sent");
        if let Err(e) = SERVER_RESTART_CHANNEL.send(()) {
            log::error!("Error could not send product changes due to {e}")
        }
    }
}
