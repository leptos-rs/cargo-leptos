use crate::{
    config::{hash_file::HashFile, lib_package::LibPackage},
    ext::{PackageExt, Paint, PathBufExt, PathExt},
    internal_prelude::*,
    logger::GRAY,
    service::site::Site,
};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::{Metadata, Package};
use serde::Deserialize;
use std::{collections::HashSet, fmt::Debug, net::SocketAddr, sync::Arc};

use super::{
    assets::AssetsConfig,
    bin_package::BinPackage,
    cli::Opts,
    dotenvs::{load_dotenvs, overlay_env},
    end2end::End2EndConfig,
    style::StyleConfig,
};

/// If the site root path starts with this marker, the marker should be replaced with the Cargo target directory
const CARGO_TARGET_DIR_MARKER: &str = "CARGO_TARGET_DIR";
/// If the site root path starts with this marker, the marker should be replaced with the Cargo target directory
const CARGO_BUILD_TARGET_DIR_MARKER: &str = "CARGO_BUILD_TARGET_DIR";

pub struct Project {
    /// absolute path to the working dir
    pub working_dir: Utf8PathBuf,
    pub name: String,
    pub lib: LibPackage,
    pub bin: BinPackage,
    pub style: StyleConfig,
    pub watch: bool,
    pub release: bool,
    pub precompress: bool,
    pub hot_reload: bool,
    pub wasm_debug: bool,
    pub site: Arc<Site>,
    pub end2end: Option<End2EndConfig>,
    pub assets: Option<AssetsConfig>,
    pub js_dir: Utf8PathBuf,
    pub watch_additional_files: Vec<Utf8PathBuf>,
    pub hash_file: HashFile,
    pub hash_files: bool,
    pub js_minify: bool,
    pub split: bool,
    pub server_fn_prefix: Option<String>,
    pub disable_server_fn_hash: bool,
    pub disable_erase_components: bool,
    pub always_erase_components: bool,
    pub server_fn_mod_path: bool,
    pub wasm_opt_features: Option<HashSet<String>>,
    pub build_frontend_only: bool,
    pub build_server_only: bool,
    pub clear_terminal_on_rebuild: bool,
}

impl Debug for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Project")
            .field("name", &self.name)
            .field("lib", &self.lib)
            .field("bin", &self.bin)
            .field("style", &self.style)
            .field("watch", &self.watch)
            .field("release", &self.release)
            .field("precompress", &self.precompress)
            .field("js_minify", &self.js_minify)
            .field("split", &self.split)
            .field("hot_reload", &self.hot_reload)
            .field("site", &self.site)
            .field("end2end", &self.end2end)
            .field("assets", &self.assets)
            .field("server_fn_prefix", &self.server_fn_prefix)
            .field("disable_server_fn_hash", &self.disable_server_fn_hash)
            .field("disable_erase_components", &self.disable_erase_components)
            .field("always_erase_components", &self.always_erase_components)
            .field("server_fn_mod_path", &self.server_fn_mod_path)
            .field("wasm_opt_features", &self.wasm_opt_features)
            .finish_non_exhaustive()
    }
}

impl Project {
    pub fn resolve(
        cli: &Opts,
        cwd: &Utf8Path,
        metadata: &Metadata,
        watch: bool,
        bin_args: Option<&[String]>,
    ) -> Result<Vec<Arc<Project>>> {
        let projects = ProjectDefinition::parse(metadata)?;

        let mut resolved = Vec::new();
        for (project, mut config) in projects {
            if config.output_name.is_empty() {
                config.output_name = project.name.to_string();
            }

            let lib = LibPackage::resolve(cli, metadata, &project, &config)?;

            let js_dir = config
                .js_dir
                .clone()
                .unwrap_or_else(|| Utf8PathBuf::from("src"));

            let watch_additional_files = config.watch_additional_files.clone().unwrap_or_default();

            let bin = BinPackage::resolve(cli, metadata, &project, &config, bin_args)?;

            // If there's more than 1 workspace member, we're a workspace. Probably
            let is_workspace = metadata.workspace_members.len() > 1;
            debug!("Detected Workspace: {is_workspace}");
            let hash_file = match is_workspace {
                true => HashFile::new(
                    Some(&metadata.workspace_root),
                    &bin,
                    config.hash_file_name.as_ref(),
                ),
                false => HashFile::new(None, &bin, config.hash_file_name.as_ref()),
            };

            let proj = Project {
                working_dir: metadata.workspace_root.clone(),
                name: project.name.clone(),
                lib,
                bin,
                style: StyleConfig::new(&config)?,
                watch,
                release: cli.release,
                precompress: cli.precompress,
                hot_reload: cli.hot_reload,
                wasm_debug: cli.wasm_debug,
                clear_terminal_on_rebuild: cli.clear,
                site: Arc::new(Site::new(&config)),
                end2end: End2EndConfig::resolve(&config),
                assets: AssetsConfig::resolve(&config),
                js_dir,
                watch_additional_files,
                hash_file,
                hash_files: config.hash_files,
                js_minify: cli.release && (cli.js_minify || config.js_minify),
                split: cli.split,
                server_fn_prefix: config.server_fn_prefix,
                disable_server_fn_hash: config.disable_server_fn_hash,
                disable_erase_components: config.disable_erase_components,
                always_erase_components: config.always_erase_components,
                server_fn_mod_path: config.server_fn_mod_path,
                wasm_opt_features: config.wasm_opt_features,
                build_frontend_only: cli.frontend_only,
                build_server_only: cli.server_only,
            };
            resolved.push(Arc::new(proj));
        }

        let projects_in_cwd = resolved
            .iter()
            .filter(|p| p.bin.abs_dir.starts_with(cwd) || p.lib.abs_dir.starts_with(cwd))
            .collect::<Vec<_>>();

        if projects_in_cwd.len() == 1 {
            Ok(vec![projects_in_cwd[0].clone()])
        } else {
            Ok(resolved)
        }
    }

    /// env vars to use when running external command
    pub fn to_envs(&self, wasm: bool) -> Vec<(&'static str, String)> {
        let mut vec = vec![
            ("LEPTOS_OUTPUT_NAME", self.lib.output_name.to_string()),
            ("LEPTOS_SITE_ROOT", self.site.root_dir.to_string()),
            ("LEPTOS_SITE_PKG_DIR", self.site.pkg_dir.to_string()),
            ("LEPTOS_SITE_ADDR", self.site.addr.to_string()),
            ("LEPTOS_RELOAD_PORT", self.site.reload.port().to_string()),
            ("LEPTOS_LIB_DIR", self.lib.rel_dir.to_string()),
            ("LEPTOS_BIN_DIR", self.bin.rel_dir.to_string()),
            ("LEPTOS_JS_MINIFY", self.js_minify.to_string()),
            ("LEPTOS_HASH_FILES", self.hash_files.to_string()),
        ];
        if self.hash_files {
            vec.push(("LEPTOS_HASH_FILE_NAME", self.hash_file.rel.to_string()));
        }
        if self.watch {
            vec.push(("LEPTOS_WATCH", self.watch.to_string()))
        }
        if let Some(prefix) = self.server_fn_prefix.as_ref() {
            vec.push(("SERVER_FN_PREFIX", prefix.clone()));
        }
        if self.disable_server_fn_hash {
            vec.push((
                "DISABLE_SERVER_FN_HASH",
                self.disable_server_fn_hash.to_string(),
            ));
        }
        if self.server_fn_mod_path {
            vec.push(("SERVER_FN_MOD_PATH", self.server_fn_mod_path.to_string()));
        }

        // add -Clink-args=--emit-relocs for wasm-splitting
        let mut additional_rustflags = String::new();
        if wasm && self.split {
            additional_rustflags.push_str(" -Clink-args=--emit-relocs");
        }

        // Set the default to erase-components mode if in debug mode and not explicitly disabled
        // or always enabled
        if (!self.disable_erase_components && !self.release) || (self.always_erase_components) {
            additional_rustflags.push_str(" --cfg erase_components");
        }

        if !additional_rustflags.is_empty() {
            let config = cargo_config2::Config::load().expect("Valid config file");
            let rustflags = if wasm {
                config.rustflags("wasm32-unknown-unknown")
            } else {
                config.rustflags(target_lexicon::HOST.to_string())
            };

            if let Ok(Some(rustflags)) = rustflags {
                let _ = rustflags
                    .encode_space_separated()
                    .inspect(|rustflags| {
                        vec.push(("RUSTFLAGS", format!("{rustflags}{additional_rustflags}")))
                    })
                    .inspect_err(|err| error!("Failed to set 'RUSTFLAGS': {}", err));
            } else {
                vec.push(("RUSTFLAGS", additional_rustflags.trim().to_string()))
            }
        }

        vec
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct ProjectConfig {
    #[serde(default)]
    pub output_name: String,
    #[serde(default = "default_site_addr")]
    pub site_addr: SocketAddr,
    #[serde(default = "default_site_root")]
    pub site_root: Utf8PathBuf,
    #[serde(default = "default_pkg_dir")]
    pub site_pkg_dir: Utf8PathBuf,
    pub style_file: Option<Utf8PathBuf>,
    /// text file where the hashes of the frontend files are stored
    pub hash_file_name: Option<Utf8PathBuf>,
    /// whether to hash the frontend files content and add them to the file names
    #[serde(default = "default_hash_files")]
    pub hash_files: bool,
    pub tailwind_input_file: Option<Utf8PathBuf>,
    pub tailwind_config_file: Option<Utf8PathBuf>,
    /// assets dir. content will be copied to the target/site dir
    pub assets_dir: Option<Utf8PathBuf>,
    /// js dir. changes triggers rebuilds.
    pub js_dir: Option<Utf8PathBuf>,
    #[serde(default = "default_js_minify")]
    pub js_minify: bool,
    /// additional files to watch. changes triggers rebuilds.
    pub watch_additional_files: Option<Vec<Utf8PathBuf>>,
    #[serde(default = "default_reload_port")]
    pub reload_port: u16,
    /// command for launching end-2-end integration tests
    pub end2end_cmd: Option<String>,
    /// the dir used when launching end-2-end integration tests
    pub end2end_dir: Option<Utf8PathBuf>,
    #[serde(default = "default_browserquery")]
    pub browserquery: String,
    /// the bin target to use for building the server
    #[serde(default)]
    pub bin_target: String,
    /// the bin output target triple to use for building the server
    pub bin_target_triple: Option<String>,
    /// the directory to put the generated server artifacts
    pub bin_target_dir: Option<String>,
    /// the command to run instead of "cargo" when building the server
    pub bin_cargo_command: Option<String>,
    /// cargo flags to pass to cargo when running the server. Overriden by bin_cargo_command
    pub bin_cargo_args: Option<Vec<String>>,
    /// An optional override, if you've changed the name of your bin file in your project you'll need to set it here as well.
    pub bin_exe_name: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub lib_features: Vec<String>,
    #[serde(default)]
    pub lib_default_features: bool,
    /// cargo flags to pass to cargo when building the WASM frontend
    pub lib_cargo_args: Option<Vec<String>>,
    #[serde(default)]
    pub bin_features: Vec<String>,
    #[serde(default)]
    pub bin_default_features: bool,

    /// The default prefix to use for server functions when generating API routes. Can be
    /// overridden for individual functions using `#[server(prefix = "...")]` as usual.
    ///
    /// This is useful to override the default prefix (`/api`) for all server functions without
    /// needing to manually specify via `#[server(prefix = "...")]` on every server function.
    #[serde(default)]
    pub server_fn_prefix: Option<String>,

    /// Whether to disable appending the server functions' hashes to the end of their API names.
    ///
    /// This is useful when an app's client side needs a stable server API. For example, shipping
    /// the CSR WASM binary in a Tauri app. Tauri app releases are dependent on each platform's
    /// distribution method (e.g., the Apple App Store or the Google Play Store), which typically
    /// are much slower than the frequency at which a website can be updated. In addition, it's
    /// common for users to not have the latest app version installed. In these cases, the CSR WASM
    /// app would need to be able to continue calling the backend server function API, so the API
    /// path needs to be consistent and not have a hash appended.
    #[serde(default)]
    pub disable_server_fn_hash: bool,

    /// Whether to disable erased components mode for debug mode. Overridden by the the following
    /// cli flag `always_enable_erase_components`.
    ///
    /// erase_components mode offers a signifigant compile time speedup by type erasing the types
    /// in your app. This is similar to adding `.into_any()` to your entire app. It can also solve
    /// some issues with compilation in debug mode. It is automatically enabled in debug mode, but
    /// can be disabled by setting this to true. If you'd like to use it for all profiles, see the
    /// next flag, `always_enable_erase_components`
    #[serde(default)]
    pub disable_erase_components: bool,

    /// Whether to enable erased components mode for all cargo-leptos builds. Overrides the cli
    /// flag `disable_erase_components`.
    ///
    /// erase_components mode offers a signifigant compile time speedup by type erasing the types
    /// in your app. This is similar to adding `.into_any()` to your entire app. It can also solve
    /// some issues with compilation in debug mode. It is automatically enabled in debug mode, but
    /// can be disabled by setting this to true. If you'd like to use it for all profiles, see the
    /// next flag, `always_enable_erase_components`
    #[serde(default)]
    pub always_erase_components: bool,
    /// Include the module path of the server function in the API route. This is an alternative
    /// strategy to prevent duplicate server function API routes (the default strategy is to add
    /// a hash to the end of the route). Each element of the module path will be separated by a `/`.
    /// For example, a server function with a fully qualified name of `parent::child::server_fn`
    /// would have an API route of `/api/parent/child/server_fn` (possibly with a
    /// different prefix and a hash suffix depending on the values of the other server fn configs).
    #[serde(default)]
    server_fn_mod_path: bool,

    #[serde(skip)]
    pub config_dir: Utf8PathBuf,
    #[serde(skip)]
    pub tmp_dir: Utf8PathBuf,

    /// Deprecated. Keeping this here to warn users to remove it in case they have it in their config.
    #[deprecated = "This option is deprecated since cargo-leptos 0.2.3 (when it became unconditionally enabled). You may remove it from your config."]
    pub separate_front_target_dir: Option<bool>,

    // Profiles
    pub lib_profile_dev: Option<String>,
    pub lib_profile_release: Option<String>,
    pub bin_profile_dev: Option<String>,
    pub bin_profile_release: Option<String>,
    pub wasm_opt_features: Option<HashSet<String>>,
}

impl ProjectConfig {
    /*fn parse(
        dir: &Utf8Path,
        metadata: &serde_json::Value,
        cargo_metadata: &Metadata,
    ) -> Result<Self> {
        let mut conf: ProjectConfig = serde_json::from_value(metadata.clone())?;
        Self::parse_raw(dir, &mut conf, cargo_metadata)?;
        Ok(conf)
    }*/

    fn parse_raw(dir: &Utf8Path, conf: &mut Self, cargo_metadata: &Metadata) -> Result<()> {
        conf.config_dir = dir.to_path_buf();
        conf.tmp_dir = cargo_metadata.target_directory.join("tmp");
        let dotenvs = load_dotenvs(dir)?;
        overlay_env(conf, dotenvs)?;
        if conf.site_root == "/"
            || conf.site_root == "."
            || conf.site_root == CARGO_TARGET_DIR_MARKER
            || conf.site_root == CARGO_BUILD_TARGET_DIR_MARKER
        {
            bail!(
                "site-root cannot be '{}'. All the content is erased when building the site.",
                conf.site_root
            );
        }
        if conf.site_root.starts_with(CARGO_TARGET_DIR_MARKER) {
            conf.site_root = {
                let mut path = cargo_metadata.target_directory.clone();
                // unwrap() should be safe because we just checked
                let sub = conf
                    .site_root
                    .unbase(CARGO_TARGET_DIR_MARKER.into())
                    .unwrap();
                path.push(sub);
                path
            };
        }
        if conf.site_root.starts_with(CARGO_BUILD_TARGET_DIR_MARKER) {
            conf.site_root = {
                let mut path = cargo_metadata.target_directory.clone();
                // unwrap() should be safe because we just checked
                let sub = conf
                    .site_root
                    .unbase(CARGO_BUILD_TARGET_DIR_MARKER.into())
                    .unwrap();
                path.push(sub);
                path
            };
        }
        if conf.site_addr.port() == conf.reload_port {
            bail!(
                "The site-addr port and reload-port cannot be the same: {}",
                conf.reload_port
            );
        }

        #[allow(deprecated)]
        if conf.separate_front_target_dir.is_some() {
            warn!("Deprecated: the `separate-front-target-dir` option is deprecated since cargo-leptos 0.2.3");
            warn!("It is now unconditionally enabled; you can remove it from your Cargo.toml")
        }

        Ok(())
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct LeptosMetadataWorkspaceSection {
    #[serde(flatten)]
    def: ProjectDefinition,
    #[serde(flatten)]
    conf: ProjectConfig,
    #[serde(flatten)]
    extra: std::collections::BTreeMap<String, serde::de::IgnoredAny>,
}
impl LeptosMetadataWorkspaceSection {
    fn check(&self) {
        if !self.extra.is_empty() {
            unused_metadata_warning(self.extra.keys().collect());
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct LeptosMetadataPackage {
    #[serde(flatten)]
    conf: ProjectConfig,
    #[serde(flatten)]
    extra: std::collections::BTreeMap<String, serde::de::IgnoredAny>,
}
impl LeptosMetadataPackage {
    fn check(&self) {
        if !self.extra.is_empty() {
            unused_metadata_warning(self.extra.keys().collect());
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProjectDefinition {
    name: String,
    pub bin_package: String,
    pub lib_package: String,
}
impl ProjectDefinition {
    fn from_workspace(
        metadata: &serde_json::Value,
        dir: &Utf8Path,
        cargo_metadata: &Metadata,
    ) -> Result<Vec<(Self, ProjectConfig)>> {
        let mut found = Vec::new();
        if let Some(arr) = metadata.as_array() {
            for section in arr {
                let mut p = LeptosMetadataWorkspaceSection::deserialize(section)?;
                p.check();
                ProjectConfig::parse_raw(dir, &mut p.conf, cargo_metadata)?;
                found.push((p.def, p.conf))
            }
        }
        Ok(found)
    }

    fn from_project(
        package: &Package,
        metadata: &serde_json::Value,
        dir: &Utf8Path,
        cargo_metadata: &Metadata,
    ) -> Result<(Self, ProjectConfig)> {
        let p = LeptosMetadataPackage::deserialize(metadata)?;
        p.check();
        let mut conf = p.conf;
        ProjectConfig::parse_raw(dir, &mut conf, cargo_metadata)?;

        ensure!(
            package.cdylib_target().is_some(),
            "Cargo.toml has leptos metadata but is missing a cdylib library target. {}",
            GRAY.paint(package.manifest_path.as_str())
        );
        ensure!(
            package.has_bin_target(),
            "Cargo.toml has leptos metadata but is missing a bin target. {}",
            GRAY.paint(package.manifest_path.as_str())
        );

        Ok((
            ProjectDefinition {
                name: package.name.to_string(),
                bin_package: package.name.to_string(),
                lib_package: package.name.to_string(),
            },
            conf,
        ))
    }

    fn parse(metadata: &Metadata) -> Result<Vec<(Self, ProjectConfig)>> {
        let workspace_dir = &metadata.workspace_root;
        let mut found: Vec<(Self, ProjectConfig)> =
            if let Some(md) = leptos_metadata(&metadata.workspace_metadata) {
                Self::from_workspace(md, &Utf8PathBuf::default(), metadata)?
            } else {
                Default::default()
            };

        for package in metadata.workspace_packages() {
            let dir = package.manifest_path.unbase(workspace_dir)?.without_last();

            if let Some(leptos_metadata) = leptos_metadata(&package.metadata) {
                found.push(Self::from_project(
                    package,
                    leptos_metadata,
                    &dir,
                    metadata,
                )?);
            }
        }
        Ok(found)
    }
}

fn leptos_metadata(metadata: &serde_json::Value) -> Option<&serde_json::Value> {
    metadata.as_object().and_then(|o| o.get("leptos"))
}

fn default_site_addr() -> SocketAddr {
    SocketAddr::new([127, 0, 0, 1].into(), 3000)
}

fn default_pkg_dir() -> Utf8PathBuf {
    Utf8PathBuf::from("pkg")
}

fn default_site_root() -> Utf8PathBuf {
    Utf8PathBuf::from(CARGO_TARGET_DIR_MARKER).join("site")
}

fn default_reload_port() -> u16 {
    3001
}

fn default_browserquery() -> String {
    "defaults".to_string()
}

fn default_hash_files() -> bool {
    false
}

fn default_js_minify() -> bool {
    true
}

fn unused_metadata_warning(keys: Vec<&String>) {
    warn!(
        "Metadata keys {:?} from metadata.leptos are not recognized and will be ignored",
        keys
    );
}
