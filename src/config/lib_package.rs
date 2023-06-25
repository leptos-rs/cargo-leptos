use crate::{
    config::Opts,
    ext::{
        anyhow::{anyhow, Result},
        MetadataExt, PathBufExt, PathExt,
    },
    service::site::{SiteFile, SourcedSiteFile},
};
use camino::Utf8PathBuf;
use cargo_metadata::Metadata;

use super::{project::ProjectDefinition, Profile, ProjectConfig};

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
    pub profile: Profile,
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

        let mut features = if !cli.lib_features.is_empty() {
            cli.lib_features.clone()
        } else if !config.lib_features.is_empty() {
            config.lib_features.clone()
        } else {
            vec![]
        };

        features.extend(config.features.clone());
        features.extend(cli.features.clone());

        let abs_dir = package.manifest_path.clone().without_last();
        let rel_dir = abs_dir.unbase(&metadata.workspace_root)?;
        let profile = Profile::new(
            cli.release,
            &config.lib_profile_release,
            &config.lib_profile_dev,
        );

        let wasm_file = {
            let source = metadata
                .rel_target_dir()
                .join("front")
                .join("wasm32-unknown-unknown")
                .join(profile.to_string())
                .join(name.replace('-', "_"))
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
        if rel_dir == "." {
            src_deps.push("src".into());
        } else {
            src_deps.push(rel_dir.join("src"));
        }
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
            profile,
        })
    }
}

impl std::fmt::Debug for LibPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibPackage")
            .field("name", &self.name)
            .field("rel_dir", &self.rel_dir.test_string())
            .field("wasm_file", &self.wasm_file)
            .field("js_file", &self.js_file)
            .field("features", &self.features)
            .field("default_features", &self.default_features)
            .field("output_name", &self.output_name)
            .field(
                "src_paths",
                &self
                    .src_paths
                    .iter()
                    .map(|p| p.test_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}
