//! This module began as a port of the wasm-split-prototype first developed
//! by @jbms at https://github.com/jbms/wasm-split-prototype
//! under the Apache License: https://github.com/jbms/wasm-split-prototype/blob/main/LICENSE

use crate::{config::Project, internal_prelude::*};
use camino::{Utf8Path, Utf8PathBuf};

pub async fn wasm_split(
    input_wasm: &[u8],
    verbose: bool,
    proj: &Project,
) -> Result<Vec<Utf8PathBuf>> {
    let dest_file = &proj.lib.wasm_file.dest;
    let dest_dir = dest_file.parent().expect("no destination directory");
    let source_file = &proj.lib.wasm_file.source;
    let main_module = &format!("/pkg/{}.js", proj.lib.output_name);

    let mut main_out_file = source_file.clone();
    main_out_file.set_file_name(format!("{}_split.wasm", source_file.file_stem().unwrap()));
    let main_out_file = main_out_file;

    let split_wasm = wasm_split_cli_support::transform(wasm_split_cli_support::Options {
        input_wasm,
        output_dir: dest_dir.as_std_path(),
        main_out_path: main_out_file.as_std_path(),
        main_module,
        link_name: Utf8Path::new("__wasm_split.______________________.js").as_std_path(),
        verbose,
    })?;
    tokio::fs::write(
        dest_dir.join("__wasm_split_manifest.json"),
        serde_json::to_string_pretty(&split_wasm.prefetch_map)
            .expect("could not serialize manifest file"),
    )
    .await?;
    return Ok(split_wasm
        .split_modules
        .into_iter()
        .map(|path| path.try_into().unwrap())
        .collect());
}
