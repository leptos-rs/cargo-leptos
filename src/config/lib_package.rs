use crate::{
    ext::{
        anyhow::{anyhow, Result},
        MetadataExt, PathBufExt, PathExt,
    },
    service::site::{SiteFile, SourcedSiteFile},
    Opts,
};
use camino::Utf8PathBuf;
use cargo_metadata::Metadata;

use super::{project::ProjectDefinition, ProjectConfig};

pub struct LibPackage {
    pub name: String,
    pub dir: Utf8PathBuf,
    pub wasm_file: SourcedSiteFile,
    pub js_file: SiteFile,
    pub features: Vec<String>,
    pub default_features: bool,
    pub output_name: String,
}

impl LibPackage {
    pub fn resolve(
        cli: &Opts,
        metadata: &Metadata,
        project: &ProjectDefinition,
        config: &ProjectConfig,
    ) -> Result<Self> {
        let name = project.lib_package.clone();
        let packages = metadata.workspace_packages();
        let output_name = if !config.output_name.is_empty() {
            config.output_name.clone()
        } else {
            name.replace('-', "_")
        };

        let package = packages
            .iter()
            .find(|p| p.name == *name)
            .ok_or_else(|| anyhow!(r#"Could not find the project lib-package "{name}""#,))?;

        println!(
            "FRONT PATHDEPS: {:?}",
            metadata.src_path_dependencies(&metadata.workspace_root, &package.id)
        );

        let features = if !config.lib_features.is_empty() {
            config.lib_features.clone()
        } else if !cli.lib_features.is_empty() {
            cli.lib_features.clone()
        } else {
            vec![]
        };

        let root = metadata.workspace_root.clone();
        let dir = package.manifest_path.clone().without_last().unbase(&root)?;
        let profile = cli.profile();

        let wasm_file = {
            let source = metadata
                .rel_target_dir()
                .join("front")
                .join("wasm32-unknown-unknown")
                .join(&profile)
                .join(&name.replace('-', "_"))
                .with_extension("wasm");
            let site = config
                .site_pkg_dir
                .join(&output_name)
                .with_extension("wasm");
            let dest = config.site_root.join(&site);
            SourcedSiteFile { source, dest, site }
        };

        let js_file = {
            let site = config.site_pkg_dir.join(&output_name).with_extension("js");
            let dest = config.site_root.join(&site);
            SiteFile { dest, site }
        };

        Ok(Self {
            name,
            dir,
            wasm_file,
            js_file,
            features,
            default_features: config.lib_default_features,
            output_name,
        })
    }
}

impl std::fmt::Debug for LibPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibPackage")
            .field("name", &self.name)
            .field("dir", &self.dir.test_string())
            .field("wasm_file", &self.wasm_file)
            .field("js_file", &self.js_file)
            .field("features", &self.features)
            .field("default_features", &self.default_features)
            .field("output_name", &self.output_name)
            .finish()
    }
}
