use crate::{
    ext::path::PathBufExt,
    service::site::{SiteFile, SourcedSiteFile},
    Opts,
};
use camino::Utf8PathBuf;
use cargo_metadata::{Metadata, Package};

use super::ProjectConfig;

#[derive(Debug)]
pub struct ProjectPaths {
    pub lib_crate_name: String,
    /// the absolute root directory.
    pub abs_root_dir: Utf8PathBuf,
    pub site_root: Utf8PathBuf,
    pub site_pkg_dir: Utf8PathBuf,
    /// the relative (to abs_root_dir) library project dir
    pub front_dir: Utf8PathBuf,
    /// the relative (to abs_root_dir) server project dir
    pub server_dir: Utf8PathBuf,
    /// the relative (to abs_root_dir) target dir
    pub target_dir: Utf8PathBuf,
    pub wasm_file: SourcedSiteFile,
    pub js_file: SiteFile,
    pub style_file: Option<SourcedSiteFile>,
    pub assets_dir: Option<Utf8PathBuf>,
    pub cargo_bin_file: Utf8PathBuf,
}

impl ProjectPaths {
    pub fn new(
        metadata: &Metadata,
        front: &Package,
        server: &Package,
        front_config: &ProjectConfig,
        cli: &Opts,
    ) -> Self {
        let abs_root_dir = metadata.workspace_root.clone();
        let front_dir = front.manifest_path.clone().without_last();
        let server_dir = server.manifest_path.clone().without_last();
        let target_dir = metadata.target_directory.clone();
        let profile = if cli.release { "release" } else { "debug" };
        let site_root = front_config.site_root.clone();
        let site_pkg_dir = site_root.join(&front_config.site_pkg_dir);
        let lib_crate_name = front.name.replace('-', "_");

        let wasm_file = {
            let source = target_dir
                .join("front")
                .join("wasm32-unkown-unknown")
                .join(&profile)
                .join(&lib_crate_name);
            let site = front_config
                .site_pkg_dir
                .join(&front_config.package_name)
                .with_extension("wasm");
            let dest = site_root.join(&site);
            SourcedSiteFile { source, dest, site }
        };

        let js_file = {
            let site = front_config
                .site_pkg_dir
                .join(&front_config.package_name)
                .with_extension("js");
            let dest = site_root.join(&site);
            SiteFile { dest, site }
        };

        let style_file = if let Some(style_file) = &front_config.style_file {
            let source = front_dir.join(style_file);
            let site = front_config
                .site_pkg_dir
                .join(&front_config.package_name)
                .with_extension("css");
            let dest = site_root.join(&site);
            Some(SourcedSiteFile { source, dest, site })
        } else {
            None
        };

        let assets_dir = front_config
            .assets_dir
            .as_ref()
            .map(|dir| front_dir.join(dir));

        let cargo_bin_file = {
            let file_ext = if cfg!(target_os = "windows") {
                "exe"
            } else {
                ""
            };
            let bin_crate_name = server.name.replace('-', "_");
            target_dir
                .join("server")
                .join(&profile)
                .join(bin_crate_name)
                .with_extension(file_ext)
        };
        Self {
            lib_crate_name,
            abs_root_dir,
            site_root,
            site_pkg_dir,
            front_dir,
            server_dir,
            target_dir,
            wasm_file,
            js_file,
            style_file,
            assets_dir,
            cargo_bin_file,
        }
    }
}
