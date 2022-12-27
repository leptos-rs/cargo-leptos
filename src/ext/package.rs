use cargo_metadata::{Package, Target};

pub trait PackageExt {
    fn has_bin_target(&self) -> bool;
    fn bin_targets(&self) -> Box<dyn Iterator<Item = &Target> + '_>;
    fn cdylib_target(&self) -> Option<&Target>;
    fn target_list(&self) -> String;
}

impl PackageExt for Package {
    fn has_bin_target(&self) -> bool {
        self.targets.iter().find(|t| t.is_bin()).is_some()
    }

    fn bin_targets(&self) -> Box<dyn Iterator<Item = &Target> + '_> {
        Box::new(self.targets.iter().filter(|t| t.is_bin()))
    }
    fn cdylib_target(&self) -> Option<&Target> {
        let cdylib: String = "cdylib".to_string();
        self.targets
            .iter()
            .find(|t| t.crate_types.contains(&cdylib))
    }
    fn target_list(&self) -> String {
        self.targets
            .iter()
            .map(|t| format!("{} ({})", t.name, t.crate_types.join(", ")))
            .collect::<Vec<_>>()
            .join(", ")
    }
}
