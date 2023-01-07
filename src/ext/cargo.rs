use std::{collections::HashSet};

use camino::{Utf8PathBuf, Utf8Path};
use cargo_metadata::{Metadata, Package, PackageId, Resolve, Target, MetadataCommand};
use super::anyhow::Result;
use super::PathExt;

pub trait PackageExt {
    fn has_bin_target(&self) -> bool;
    fn bin_targets(&self) -> Box<dyn Iterator<Item = &Target> + '_>;
    fn cdylib_target(&self) -> Option<&Target>;
    fn target_list(&self) -> String;
    fn path_dependencies(&self) -> Vec<Utf8PathBuf>;
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

    fn path_dependencies(&self) -> Vec<Utf8PathBuf> {
        let mut found = Vec::new();
        for dep in &self.dependencies {
            if let Some(path) = &dep.path {
                found.push(path.clone());
            }
        }
        found
    }
}

pub trait MetadataExt {
    fn load_cleaned(manifest_path: &Utf8Path) -> Result<Metadata>;
    fn rel_target_dir(&self) -> Utf8PathBuf;
    fn package_for(&self, id: &PackageId) -> Option<&Package>;
    fn path_dependencies(&self, id: &PackageId) -> Vec<Utf8PathBuf>;
    fn src_path_dependencies(&self, id: &PackageId) -> Vec<Utf8PathBuf>;
}

impl MetadataExt for Metadata {

    fn load_cleaned(manifest_path: &Utf8Path) -> Result<Metadata> {
        let mut metadata = MetadataCommand::new().manifest_path(manifest_path).exec()?;
        if cfg!(windows) {
            let cleaned = dunce::simplified(metadata.workspace_root.as_ref());
            metadata.workspace_root = Utf8PathBuf::from_path_buf(cleaned.to_path_buf()).unwrap();    
        }
        Ok(metadata)
    }

    fn rel_target_dir(&self) -> Utf8PathBuf {
        self.target_directory.clone().unbase(&self.workspace_root).unwrap()
    }

    fn package_for(&self, id: &PackageId) -> Option<&Package> {
        self.packages.iter().find(|p| p.id == *id)
    }

    fn path_dependencies(&self, id: &PackageId) -> Vec<Utf8PathBuf> {
        let Some(resolve) = &self.resolve else {   
             return vec![]
        };
        let mut found = vec![];

        let mut set = HashSet::new();
        resolve.deps_for(id, &mut set);

        for pck in &self.packages {
            if set.contains(&pck.id) {
                found.extend(pck.path_dependencies())
            }
        }

        found
    }

    fn src_path_dependencies(&self, id: &PackageId) -> Vec<Utf8PathBuf> {
        let root = &self.workspace_root;
        self.path_dependencies(id).iter().map(|p| p.unbase(root).unwrap_or_else(|_| {
            println!("Warning: could not unbase path dependency {:?} from workspace root {:?}",
                &p, &root);
            p.to_path_buf()}).join("src")).collect()
    }
}

pub trait ResolveExt {
    fn deps_for(&self, id: &PackageId, set: &mut HashSet<PackageId>);
}

impl ResolveExt for Resolve {
    fn deps_for(&self, id: &PackageId, set: &mut HashSet<PackageId>) {
        if let Some(node) = self.nodes.iter().find(|n| n.id == *id) {
            if set.insert(node.id.clone()) {
                for dep in &node.deps {
                    self.deps_for(&dep.pkg, set);
                }    
            }
        }
    }
}
