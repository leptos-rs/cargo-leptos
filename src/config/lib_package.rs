use crate::{
    config::{lib_cargo_args, Opts},
    ext::{MetadataExt, PackageExt, PathBufExt, PathExt},
    internal_prelude::*,
    logger::GRAY,
    service::site::{SiteFile, SourcedSiteFile},
};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::Metadata;

use super::{
    project::{ProjectDefinition, CARGO_BUILD_TARGET_DIR_MARKER, CARGO_TARGET_DIR_MARKER},
    Profile, ProjectConfig,
};

struct FrontTargetPaths {
    rel: Utf8PathBuf,
    abs: Utf8PathBuf,
}

impl FrontTargetPaths {
    fn new(
        target_directory: &Utf8Path,
        workspace_root: &Utf8Path,
        front_target_dir: Option<&Utf8Path>,
    ) -> Self {
        let abs = match front_target_dir {
            Some(front_target_dir) => resolve_configured_front_target_dir(
                target_directory,
                workspace_root,
                front_target_dir,
            ),
            None => target_directory.join("front"),
        };
        let rel = pathdiff::diff_utf8_paths(&abs, workspace_root).unwrap_or_else(|| abs.clone());
        Self { rel, abs }
    }
}

fn resolve_configured_front_target_dir(
    target_directory: &Utf8Path,
    workspace_root: &Utf8Path,
    front_target_dir: &Utf8Path,
) -> Utf8PathBuf {
    if front_target_dir.as_str() == CARGO_TARGET_DIR_MARKER
        || front_target_dir.as_str() == CARGO_BUILD_TARGET_DIR_MARKER
    {
        return target_directory.to_path_buf();
    }

    if front_target_dir.starts_with(CARGO_TARGET_DIR_MARKER) {
        return front_target_dir
            .unbase(Utf8Path::new(CARGO_TARGET_DIR_MARKER))
            .map(|suffix| target_directory.join(suffix))
            .unwrap_or_else(|_| target_directory.to_path_buf());
    }

    if front_target_dir.starts_with(CARGO_BUILD_TARGET_DIR_MARKER) {
        return front_target_dir
            .unbase(Utf8Path::new(CARGO_BUILD_TARGET_DIR_MARKER))
            .map(|suffix| target_directory.join(suffix))
            .unwrap_or_else(|_| target_directory.to_path_buf());
    }

    if front_target_dir.is_absolute() {
        front_target_dir.to_path_buf()
    } else {
        workspace_root.join(front_target_dir)
    }
}

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
    pub front_target_path: Utf8PathBuf,
    pub profile: Profile,
    pub cargo_args: Vec<String>,
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
            .find(|p| *p.name == name)
            .ok_or_else(|| eyre!(r#"Could not find the project lib-package "{name}""#,))?;

        let Some(target_lib) = package.cdylib_target() else {
            return Err(eyre!(
                r#"Could not find a cdylib library target for the leptos lib-package "{}" defined in {}"#,
                package.name,
                GRAY.paint(package.manifest_path.as_str()),
            ));
        };

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
        let front_target_paths = FrontTargetPaths::new(
            &metadata.target_directory,
            &metadata.workspace_root,
            config.front_target_dir.as_deref(),
        );

        let wasm_file = {
            let source = front_target_paths
                .rel
                .join("wasm32-unknown-unknown")
                .join(profile.to_string())
                .join(target_lib.name.clone())
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

        let front_target_path = front_target_paths.abs;
        let cargo_args = lib_cargo_args(cli, config);

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
            front_target_path,
            profile,
            cargo_args,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_target_paths_use_dedicated_front_dir_by_default() {
        let paths = FrontTargetPaths::new(
            Utf8Path::new("/workspace/target"),
            Utf8Path::new("/workspace"),
            None,
        );

        assert_eq!(paths.rel, Utf8PathBuf::from("target/front"));
        assert_eq!(paths.abs, Utf8PathBuf::from("/workspace/target/front"));
    }

    #[test]
    fn front_target_paths_use_cargo_target_dir_marker() {
        let paths = FrontTargetPaths::new(
            Utf8Path::new("/workspace/target"),
            Utf8Path::new("/workspace"),
            Some(Utf8Path::new(CARGO_TARGET_DIR_MARKER)),
        );

        assert_eq!(paths.rel, Utf8PathBuf::from("target"));
        assert_eq!(paths.abs, Utf8PathBuf::from("/workspace/target"));
    }

    #[test]
    fn front_target_paths_resolve_workspace_relative_override() {
        let paths = FrontTargetPaths::new(
            Utf8Path::new("/workspace/target"),
            Utf8Path::new("/workspace"),
            Some(Utf8Path::new("custom/front-target")),
        );

        assert_eq!(paths.rel, Utf8PathBuf::from("custom/front-target"));
        assert_eq!(
            paths.abs,
            Utf8PathBuf::from("/workspace/custom/front-target")
        );
    }
}
