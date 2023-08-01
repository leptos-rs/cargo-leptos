use super::ProjectConfig;
use crate::ext::anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use std::{env, fs};
use crate::ext::exe;

pub fn load_dotenvs(directory: &Utf8Path) -> Result<Option<Vec<(String, String)>>> {
    let candidate = directory.join(".env");

    if let Ok(metadata) = fs::metadata(&candidate) {
        if metadata.is_file() {
            let mut dotenvs = vec![];
            for entry in dotenvy::from_path_iter(&candidate)? {
                let (key, val) = entry?;
                dotenvs.push((key, val));
            }

            return Ok(Some(dotenvs));
        }
    }

    if let Some(parent) = directory.parent() {
        load_dotenvs(parent)
    } else {
        Ok(None)
    }
}

pub fn overlay_env(conf: &mut ProjectConfig, dotenvs: Option<Vec<(String, String)>>) -> Result<()> {
    if let Some(dotenvs) = dotenvs {
        overlay(conf, dotenvs.into_iter())?;
    }
    overlay(conf, env::vars())?;
    Ok(())
}

fn overlay(conf: &mut ProjectConfig, envs: impl Iterator<Item = (String, String)>) -> Result<()> {
    for (key, val) in envs {
        match key.as_str() {
            "LEPTOS_OUTPUT_NAME" => conf.output_name = val,
            "LEPTOS_SITE_ROOT" => conf.site_root = Utf8PathBuf::from(val),
            "LEPTOS_SITE_PKG_DIR" => conf.site_pkg_dir = Utf8PathBuf::from(val),
            "LEPTOS_STYLE_FILE" => conf.style_file = Some(Utf8PathBuf::from(val)),
            "LEPTOS_ASSETS_DIR" => conf.assets_dir = Some(Utf8PathBuf::from(val)),
            "LEPTOS_SITE_ADDR" => conf.site_addr = val.parse()?,
            "LEPTOS_RELOAD_PORT" => conf.reload_port = val.parse()?,
            "LEPTOS_END2END_CMD" => conf.end2end_cmd = Some(val),
            "LEPTOS_END2END_DIR" => conf.end2end_dir = Some(Utf8PathBuf::from(val)),
            "LEPTOS_BROWSERQUERY" => conf.browserquery = val,
            "LEPTOS_BIN_TARGET_TRIPLE" => conf.bin_target_triple = Some(val),
            "LEPTOS_BIN_TARGET_DIR" => conf.bin_target_dir = Some(val),
            "LEPTOS_BIN_CARGO_COMMAND" => conf.bin_cargo_command = Some(val),
            // put these here to suppress the warning, but there's no
            // good way at the moment to pull the ProjectConfig all the way to Exe
            exe::ENV_VAR_LEPTOS_TAILWIND_VERSION => {},
            exe::ENV_VAR_LEPTOS_SASS_VERSION => {},
            exe::ENV_VAR_LEPTOS_CARGO_GENERATE_VERSION => {},
            exe::ENV_VAR_LEPTOS_WASM_OPT_VERSION => {},
            _ if key.starts_with("LEPTOS_") => {
                log::warn!("Env {key} is not used by cargo-leptos")
            }
            _ => {}
        }
    }
    Ok(())
}
