use std::vec;

use crate::service::notify::Watched;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// sent when a bin target source file is changed
    BinSource,
    /// sent when a lib target source file is changed
    LibSource,
    /// sent when an asset file changed
    Asset(Watched),
    /// sent when a style file changed
    Style,
    /// Cargo.toml changed
    Conf,
    /// Additionnal file changed
    Additionnal,
}

#[derive(Debug, Default, Clone)]
pub struct ChangeSet(Vec<Change>);

impl ChangeSet {
    pub fn all_changes() -> Self {
        Self(vec![
            Change::BinSource,
            Change::LibSource,
            Change::Style,
            Change::Conf,
            Change::Asset(Watched::Rescan),
        ])
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn clear(&mut self) {
        self.0.clear()
    }

    pub fn need_server_build(&self) -> bool {
        self.0.contains(&Change::BinSource)
            || self.0.contains(&Change::Conf)
            || self.0.contains(&Change::Additionnal)
    }

    pub fn need_front_build(&self) -> bool {
        self.0.contains(&Change::LibSource)
            || self.0.contains(&Change::Conf)
            || self.0.contains(&Change::Additionnal)
    }

    pub fn asset_iter(&self) -> impl Iterator<Item = &Watched> {
        self.0.iter().filter_map(|change| match change {
            Change::Asset(a) => Some(a),
            _ => None,
        })
    }

    pub fn need_style_build(&self, css_files: bool, css_in_source: bool) -> bool {
        (css_files && self.0.contains(&Change::Style))
            || (css_in_source && self.0.contains(&Change::LibSource))
    }

    pub fn add(&mut self, change: Change) -> bool {
        if !self.0.contains(&change) {
            self.0.push(change);
            true
        } else {
            false
        }
    }
}
