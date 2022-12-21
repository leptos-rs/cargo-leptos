use crate::{config::Config, service::site::SiteFile};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::Package as CargoPackage;

impl Config {
    /// Get the crate name for cdylib crate
    pub fn lib_crate_name(&self) -> String {
        lib_crate_name(&self.cargo)
    }

    pub fn cargo_wasm_file(&self) -> Utf8PathBuf {
        let rel_dbg = if self.cli.release { "release" } else { "debug" };

        self.target_dir()
            .join("front")
            .join("wasm32-unknown-unknown")
            .join(rel_dbg)
            .join(self.lib_crate_name())
            .with_extension("wasm")
    }

    pub fn site_wasm_file(&self) -> SiteFile {
        SiteFile::from(
            self.leptos
                .site_pkg_dir
                .join(&self.leptos.package_name)
                .with_extension("wasm"),
        )
    }

    pub fn site_js_file(&self) -> SiteFile {
        SiteFile::from(
            self.leptos
                .site_pkg_dir
                .join(&self.leptos.package_name)
                .with_extension("js"),
        )
    }

    pub fn site_css_file(&self) -> SiteFile {
        SiteFile::from(
            self.leptos
                .site_pkg_dir
                .join(&self.leptos.package_name)
                .with_extension("css"),
        )
    }

    pub fn site_root(&self) -> &Utf8Path {
        &self.leptos.site_root
    }
    pub fn pkg_dir(&self) -> SiteFile {
        self.leptos.site_pkg_dir.clone()
    }

    /// Get the crate name for bin crate
    pub fn bin_crate_name(&self) -> String {
        match self
            .cargo
            .targets
            .iter()
            .find(|t| t.kind.iter().any(|k| k == "bin"))
        {
            Some(bin) => bin.name.replace('-', "_"),
            None => self.cargo.name.replace('-', "_"),
        }
    }

    pub fn cargo_bin_file(&self) -> Utf8PathBuf {
        let rel_dbg = if self.cli.release { "release" } else { "debug" };

        let file_ext = if cfg!(target_os = "windows") {
            "exe"
        } else {
            ""
        };
        self.target_dir()
            .join("server")
            .join(rel_dbg)
            .join(self.lib_crate_name())
            .with_extension(file_ext)
    }

    pub fn target_dir(&self) -> &Utf8Path {
        &self.workspace.target_directory
    }
}

pub fn lib_crate_name(cargo: &CargoPackage) -> String {
    match cargo
        .targets
        .iter()
        .find(|t| t.kind.iter().any(|k| k == "cdylib"))
    {
        Some(lib) => lib.name.replace('-', "_"),
        None => cargo.name.replace('-', "_"),
    }
}
