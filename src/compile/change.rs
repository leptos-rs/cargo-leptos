use std::{ops::Deref, vec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// sent when a bin target source file is changed
    BinSource,
    /// sent when a lib target source file is changed
    LibSource,
    /// sent when an asset file changed
    Asset,
    /// sent when a style file changed
    Style,
    /// Cargo.toml changed
    Conf,
    /// Additional file changed
    Additional,
}

#[derive(Debug, Default, Clone)]
pub struct ChangeSet(Vec<Change>);

impl ChangeSet {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn all_changes() -> Self {
        Self(vec![
            Change::BinSource,
            Change::LibSource,
            Change::Style,
            Change::Conf,
            Change::Asset,
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
            || self.0.contains(&Change::Additional)
    }

    pub fn need_front_build(&self) -> bool {
        self.0.contains(&Change::LibSource)
            || self.0.contains(&Change::Conf)
            || self.0.contains(&Change::Additional)
    }

    pub fn need_style_build(&self, css_files: bool, css_in_source: bool) -> bool {
        (css_files && self.0.contains(&Change::Style))
            || (css_in_source && self.0.contains(&Change::LibSource))
    }

    pub fn need_assets_change(&self) -> bool {
        self.0.contains(&Change::Asset)
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

impl Deref for ChangeSet {
    type Target = Vec<Change>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
