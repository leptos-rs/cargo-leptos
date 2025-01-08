use camino::Utf8PathBuf;
use cargo_metadata::{Metadata, Target};
use current_platform::CURRENT_PLATFORM;
use super::{project::ProjectDefinition, Profile, ProjectConfig};
use crate::{
    config::Opts,
    ext::{
        anyhow::{anyhow, bail, Error, Result},
        MetadataExt, PackageExt, PathBufExt, PathExt,
    },
};
pub struct BinPackage {
    pub name: String,
    pub abs_dir: Utf8PathBuf,
    pub rel_dir: Utf8PathBuf,
    pub exe_file: Utf8PathBuf,
    pub target: String,
    pub features: Vec<String>,
    pub default_features: bool,
    /// all source paths, including path dependencies'
    pub src_paths: Vec<Utf8PathBuf>,
    pub profile: Profile,
    pub target_triple: Option<String>,
    pub target_dir: Option<String>,
    pub cargo_command: Option<String>,
    pub cargo_args: Option<Vec<String>>,
    pub bin_args: Option<Vec<String>>,
}

impl BinPackage {
    pub fn resolve(
        cli: &Opts,
        metadata: &Metadata,
        project: &ProjectDefinition,
        config: &ProjectConfig,
        bin_args: Option<&[String]>,
    ) -> Result<Self> {
        let mut features = if !cli.bin_features.is_empty() {
            cli.bin_features.clone()
        } else if !config.bin_features.is_empty() {
            config.bin_features.clone()
        } else {
            vec![]
        };

        features.extend(config.features.clone());
        features.extend(cli.features.clone());

        let name = project.bin_package.clone();
        let packages = metadata.workspace_packages();
        let package = packages
            .iter()
            .find(|p| p.name == name && p.has_bin_target())
            .ok_or_else(|| anyhow!(r#"Could not find the project bin-package "{name}""#,))?;

        let package = (*package).clone();

        let targets = package
            .targets
            .iter()
            .filter(|t| t.is_bin())
            .collect::<Vec<&Target>>();

        let target: Target = if !&config.bin_target.is_empty() {
            targets
                .into_iter()
                .find(|t| t.name == config.bin_target)
                .ok_or_else(|| target_not_found(config.bin_target.as_str()))?
                .clone()
        } else if targets.len() == 1 {
            targets[0].clone()
        } else if targets.is_empty() {
            bail!("No bin targets found for member {name}");
        } else {
            return Err(many_targets_found(&name));
        };

        let abs_dir = package.manifest_path.clone().without_last();
        let rel_dir = abs_dir.unbase(&metadata.workspace_root)?;
        let profile = Profile::new(
            cli.release,
            &config.bin_profile_release,
            &config.bin_profile_dev,
        );
        let exe_file = {
            let file_ext = if cfg!(target_os = "windows")
                && config
                    .bin_target_triple
                    .as_ref()
                    .is_none_or(|triple| triple.contains("-pc-windows-"))
            {
                "exe"
            } else if config
                .bin_target_triple
                .as_ref()
                .is_some_and(|target| target.starts_with("wasm32-"))
            {
                "wasm"
            } else {
                ""
            };

            let mut file = config
                .bin_target_dir
                .as_ref()
                .map(|dir| dir.into())
                // Can't use absolute path because the path gets stored in snapshot testing, and it differs between developers
                .unwrap_or_else(|| metadata.rel_target_dir());
            if let Some(triple) = &config.bin_target_triple {
                file = file.join(triple)
            };
            let name = if let Some(name) = &config.bin_exe_name {
                name
            } else {
                &name
            };
            let mut test_file = file.join(profile.to_string())
                .join(name)
                .with_extension(file_ext);
            // Check if the file exists and if not, try to prepend target_triple
            // right now it mail fail to find target/debug/name
            // but the build is successful and in target/"target_triple"/debug/name
            // https://github.com/leptos-rs/cargo-leptos/issues/358
            if !test_file.exists(){
                test_file = Utf8PathBuf::from(format!(
                    "target/{}/{}/{}",
                    CURRENT_PLATFORM, profile.to_string(),test_file.file_name().unwrap()
                ));
            }
            test_file
        };

        let mut src_paths = metadata.src_path_dependencies(&package.id);
        if rel_dir == "." {
            src_paths.push("src".into());
        } else {
            src_paths.push(rel_dir.join("src"));
        }

        let cargo_args = cli
            .bin_cargo_args
            .clone()
            .or_else(|| config.bin_cargo_args.clone());

        log::debug!("BEFORE BIN {:?}", config.bin_cargo_command);
        Ok(Self {
            name,
            abs_dir,
            rel_dir,
            exe_file,
            target: target.name,
            features,
            default_features: config.bin_default_features,
            src_paths,
            profile,
            target_triple: config.bin_target_triple.clone(),
            target_dir: config.bin_target_dir.clone(),
            cargo_command: config.bin_cargo_command.clone(),
            cargo_args,
            bin_args: bin_args.map(ToOwned::to_owned),
        })
    }
}

impl std::fmt::Debug for BinPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BinPackage")
            .field("name", &self.name)
            .field("rel_dir", &self.rel_dir.test_string())
            .field("exe_file", &self.exe_file.test_string())
            .field("target", &self.target)
            .field("features", &self.features)
            .field("default_features", &self.default_features)
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
            .field("bin_args", &self.bin_args)
            .finish_non_exhaustive()
    }
}

fn many_targets_found(pkg: &str) -> Error {
    anyhow!(
        r#"Several bin targets found for member "{pkg}", please specify which one to use with: [[workspace.metadata.leptos]] bin-target = "name""#
    )
}
fn target_not_found(target: &str) -> Error {
    anyhow!(
        r#"Could not find the target specified: [[workspace.metadata.leptos]] bin-target = "{target}""#,
    )
}
