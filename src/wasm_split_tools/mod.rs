//! This module began as a port of the wasm-split-prototype first developed
//! by @jbms at https://github.com/jbms/wasm-split-prototype
//! under the Apache License: https://github.com/jbms/wasm-split-prototype/blob/main/LICENSE

mod dep_graph;
mod emit;
mod read;
mod split_point;

use crate::{config::Project, internal_prelude::*};
use camino::Utf8PathBuf;
use split_point::SplitModuleIdentifier;
use std::collections::HashMap;

pub async fn wasm_split(
    input_wasm: &[u8],
    verbose: bool,
    proj: &Project,
) -> Result<Vec<Utf8PathBuf>> {
    let mut split_files = vec![];

    let module = self::read::InputModule::parse(input_wasm)?;
    let dep_graph = dep_graph::get_dependencies(&module)?;
    let split_points = split_point::get_split_points(&module)?;
    let mut split_program_info =
        split_point::compute_split_modules(&module, &dep_graph, &split_points)?;

    if verbose {
        for (name, split_deps) in split_program_info.output_modules.iter() {
            split_deps.print(format!("{name:?}").as_str(), &module);
        }
    }

    let dest_dir = proj
        .lib
        .wasm_file
        .dest
        .parent()
        .expect("no destination directory");
    std::fs::create_dir_all(dest_dir)?;

    self::emit::emit_modules(
        &module,
        &mut split_program_info,
        |identifier: &SplitModuleIdentifier, data: &[u8], hash: &str| -> Result<()> {
            let output_path = match identifier {
                SplitModuleIdentifier::Main => proj.lib.wasm_file.source.clone(),
                _ => {
                    let name = identifier.name(proj);
                    let name_hashed = format!("{name}.{hash}");
                    dest_dir.join(name_hashed + ".wasm")
                }
            };

            std::fs::write(&output_path, data)?;

            if !matches!(identifier, SplitModuleIdentifier::Main) {
                split_files.push(output_path);
            }

            Ok(())
        },
    )?;

    let mut javascript = String::new();
    javascript.push_str(r#"import { initSync } from "/pkg/"#);
    javascript.push_str(&proj.lib.output_name);
    javascript.push_str(
        r#".js";
function makeLoad(url, deps) {
  let alreadyLoaded = false;
  return async(callbackIndex, callbackData) => {
    if (alreadyLoaded) return;
    let mainExports = undefined;
      try {
        const loadAll = await Promise.all([fetch(url), ...deps.map(dep => dep())]);
        const response = loadAll[0];
        mainExports = initSync(undefined, undefined);
        const imports = {
          env: {
            memory: mainExports.memory,
          },
          __wasm_split: {
            __indirect_function_table: mainExports.__indirect_function_table,
            __stack_pointer: mainExports.__stack_pointer,
            __tls_base: mainExports.__tls_base,
            memory: mainExports.memory,
          },
        };
        const module = await WebAssembly.instantiateStreaming(response, imports);
        alreadyLoaded = true;
        if (callbackIndex === undefined) return;
        mainExports.__indirect_function_table.get(callbackIndex)(
          callbackData,
          true,
        );
      } catch (e) {
        if (callbackIndex === undefined) throw e;
        console.error("Failed to load " + url.href, e);
        if (mainExports === undefined) {
          mainExports = initSync(undefined, undefined);
        }
        mainExports.__indirect_function_table.get(callbackIndex)(
          callbackData,
          false,
        );
      }
  };
}
"#,
    );
    let mut split_deps = HashMap::<String, Vec<String>>::new();
    for (identifier, _) in split_program_info.output_modules.iter() {
        let SplitModuleIdentifier::Chunk { splits, .. } = identifier else {
            continue;
        };
        let chunk_name_hashed = identifier.name_hashed(proj);
        let chunk_name_sanitized = chunk_name_hashed.replace(['.', '-'], "_");

        for split_name in splits {
            split_deps
                .entry(split_name.clone())
                .or_default()
                .push(chunk_name_sanitized.clone());
        }

        if !chunk_name_hashed.is_empty() {
            javascript.push_str(
                format!(
                    "const __wasm_split_load_{chunk_name_sanitized} = makeLoad(new URL(\"./{chunk_name_hashed}.wasm\", import.meta.url), []);\n",
                ).as_str()
            )
        }
    }

    for (identifier, _) in split_program_info.output_modules.iter().rev() {
        if !matches!(identifier, SplitModuleIdentifier::Split { .. }) {
            continue;
        }
        let name_hashed = identifier.name_hashed(proj);
        if !name_hashed.is_empty() {
            if let SplitModuleIdentifier::Split { name, .. } = identifier {
                javascript.push_str(
                    format!(
                        "export const __wasm_split_load_{name} = makeLoad(new URL(\"./{name_hashed}.wasm\", import.meta.url), [{deps}]);\n",
                        deps = split_deps
                            .remove(name)
                            .unwrap_or_default()
                            .iter()
                            .map(|x| format!("__wasm_split_load_{x}"))
                            .collect::<Vec<_>>()
                            .join(", "),
                    ).as_str()
                )
            }
        }
    }

    tokio::fs::write(
        proj.lib
            .wasm_file
            .dest
            .parent()
            .expect("no destination directory")
            .join("__wasm_split.js"),
        javascript,
    )
    .await?;

    Ok(split_files)
}
