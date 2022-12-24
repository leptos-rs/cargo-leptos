use super::ProjectConfig;
use crate::{ext::anyhow::Result, logger::GRAY};
use camino::{Utf8Path, Utf8PathBuf};
use std::fs;

pub fn find_env_file(directory: &Utf8Path) -> Option<Utf8PathBuf> {
    let candidate = directory.join(".env");

    if let Ok(metadata) = fs::metadata(&candidate) {
        if metadata.is_file() {
            return Some(candidate);
        }
    }

    if let Some(parent) = directory.parent() {
        find_env_file(parent)
    } else {
        None
    }
}

pub fn overlay_env(conf: &mut ProjectConfig, file: &Utf8Path) -> Result<()> {
    for entry in dotenvy::from_path_iter(file)? {
        let (key, val) = entry?;

        match key.as_str() {
            "PACKAGE_NAME" => conf.package_name = val,
            "LEPTOS_SITE_ROOT" => conf.site_root = Utf8PathBuf::from(val),
            "LEPTOS_SITE_PKG_DIR" => conf.site_pkg_dir = Utf8PathBuf::from(val),
            "LEPTOS_STYLE_FILE" => conf.style_file = Some(Utf8PathBuf::from(val)),
            "LEPTOS_ASSETS_DIR" => conf.assets_dir = Some(Utf8PathBuf::from(val)),
            "LEPTOS_SITE_ADDR" => conf.site_addr = val.parse()?,
            "LEPTOS_RELOAD_PORT" => conf.reload_port = val.parse()?,
            "LEPTOS_END2END_CMD" => conf.end2end_cmd = Some(val),
            "LEPTOS_END2END_DIR" => conf.end2end_dir = Some(val),
            "LEPTOS_BROWSERQUERY" => conf.browserquery = val,
            _ if key.starts_with("LEPTOS_") => {
                log::warn!(
                    "Env {key} is not used by cargo-leptos {}",
                    GRAY.paint(file.as_str())
                )
            }
            _ => log::debug!(
                r#"Env unused param "{key} = {val}" {}"#,
                GRAY.paint(file.as_str())
            ),
        }
    }
    Ok(())
}
