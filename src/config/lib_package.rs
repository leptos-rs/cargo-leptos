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
    /// absolute dir to package
    pub abs_dir: Utf8PathBuf,
    pub rel_dir: Utf8PathBuf,
    pub wasm_file: SourcedSiteFile,
    pub js_file: SiteFile,
    pub features: Vec<String>,
    pub default_features: bool,
    pub output_name: String,
    pub src_paths: Vec<Utf8PathBuf>,
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

        let features = if !config.lib_features.is_empty() {
            config.lib_features.clone()
        } else if !cli.lib_features.is_empty() {
            cli.lib_features.clone()
        } else {
            vec![]
        };

        let abs_dir = package.manifest_path.clone().without_last();
        let rel_dir = abs_dir.unbase(&metadata.workspace_root)?;
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

        let mut src_deps = metadata.src_path_dependencies(&package.id);
        src_deps.push(rel_dir.join("src"));
        Ok(Self {
            name,
            abs_dir,
            rel_dir,
            wasm_file,
            js_file,
            features,
            default_features: config.lib_default_features,
            output_name,
            src_paths: src_deps,
        })
    }
}

impl std::fmt::Debug for LibPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibPackage")
            .field("name", &self.name)
            .field("rel_dir", &self.rel_dir)
            .field("wasm_file", &self.wasm_file)
            .field("js_file", &self.js_file)
            .field("features", &self.features)
            .field("default_features", &self.default_features)
            .field("output_name", &self.output_name)
            .field(
                "path_deps",
                &self
                    .src_paths
                    .iter()
                    .map(|p| p.test_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
            .finish_non_exhaustive()
    }
}
