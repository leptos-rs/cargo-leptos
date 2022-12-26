use crate::{
    ext::anyhow::Result,
    ext::path::{PathBufExt, PathExt},
    logger::GRAY,
    service::site::{SiteFile, SourcedSiteFile},
    Opts,
};
use anyhow::ensure;
use camino::Utf8PathBuf;
use cargo_metadata::{Metadata, Package};

use super::ProjectConfig;

#[cfg_attr(not(test), derive(Debug))]
pub struct ProjectPaths {
    pub lib_crate_name: String,
    /// the absolute root directory.
    pub abs_root_dir: Utf8PathBuf,
    /// the dir of the ProjectConfig relative to workspace
    // pub config_dir: Utf8PathBuf,
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

#[cfg(test)]
impl std::fmt::Debug for ProjectPaths {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectPaths")
            .field("lib_crate_name", &self.lib_crate_name)
            .field("site_root", &self.site_root.test_string())
            .field(
                "site_pkg_dir",
                &self.site_pkg_dir.as_str().replace("\\", "/"),
            )
            .field("front_dir", &self.front_dir.test_string())
            .field("server_dir", &self.server_dir.test_string())
            .field("target_dir", &self.target_dir.test_string())
            .field("wasm_file", &self.wasm_file)
            .field("js_file", &self.js_file)
            .field("style_file", &self.style_file)
            .field(
                "assets_dir",
                &self.assets_dir.as_ref().map(|d| d.test_string()),
            )
            .field("cargo_bin_file", &self.cargo_bin_file.test_string())
            .finish_non_exhaustive()
    }
}
impl ProjectPaths {
    pub fn validate(self) -> Result<Self> {
        ensure!(
            self.abs_root_dir.join(&self.front_dir).exists(),
            "The frontend package dir doesn't exist {}",
            GRAY.paint(self.front_dir.as_str())
        );
        ensure!(
            self.abs_root_dir.join(&self.server_dir).exists(),
            "The server package dir doesn't exist {}",
            GRAY.paint(self.server_dir.as_str())
        );
        if let Some(style_file) = &self.style_file {
            ensure!(
                self.abs_root_dir.join(&style_file.source).exists(),
                "The style file doesn't exist {}",
                GRAY.paint(style_file.source.as_str())
            );
        }
        if let Some(assets_dir) = &self.assets_dir {
            ensure!(
                self.abs_root_dir.join(&assets_dir).exists(),
                "The assets directory doesn't exist {}",
                GRAY.paint(assets_dir.as_str())
            );
        }
        Ok(self)
    }

    pub fn new(
        metadata: &Metadata,
        front: &Package,
        server: &Package,
        front_config: &ProjectConfig,
        cli: &Opts,
    ) -> Result<Self> {
        let abs_root_dir = metadata.workspace_root.clone();
        log::trace!("Project root dir {abs_root_dir}");
        let front_dir = front
            .manifest_path
            .clone()
            .without_last()
            .unbase(&abs_root_dir)?;
        let server_dir = server
            .manifest_path
            .clone()
            .without_last()
            .unbase(&abs_root_dir)?;
        let target_dir = metadata.target_directory.clone().unbase(&abs_root_dir)?;
        let profile = if cli.release { "release" } else { "debug" };
        let site_root = front_config.site_root.clone();
        let site_pkg_dir = site_root.join(&front_config.site_pkg_dir);
        let lib_crate_name = front.name.replace('-', "_");

        let wasm_file = {
            let source = target_dir
                .join("front")
                .join("wasm32-unknown-unknown")
                .join(&profile)
                .join(&lib_crate_name)
                .with_extension("wasm");
            let site = front_config
                .site_pkg_dir
                .join(&front_config.output_name)
                .with_extension("wasm");
            let dest = site_root.join(&site);
            SourcedSiteFile { source, dest, site }
        };

        let js_file = {
            let site = front_config
                .site_pkg_dir
                .join(&front_config.output_name)
                .with_extension("js");
            let dest = site_root.join(&site);
            SiteFile { dest, site }
        };

        let style_file = if let Some(style_file) = &front_config.style_file {
            // relative to the configuration file
            let source = front_config.config_dir.join(style_file);
            let site = front_config
                .site_pkg_dir
                .join(&front_config.output_name)
                .with_extension("css");
            let dest = site_root.join(&site);
            Some(SourcedSiteFile { source, dest, site })
        } else {
            None
        };

        // relative to the configuration file
        let assets_dir = front_config
            .assets_dir
            .as_ref()
            .map(|dir| front_config.config_dir.join(dir));

        let cargo_bin_file = {
            let file_ext = if cfg!(target_os = "windows") {
                "exe"
            } else {
                ""
            };
            let bin_crate_name = server.name.clone();
            target_dir
                .join("server")
                .join(&profile)
                .join(bin_crate_name)
                .with_extension(file_ext)
        };
        Ok(Self {
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
        })
        // .validate()
    }
}
