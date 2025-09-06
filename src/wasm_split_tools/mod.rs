//! This module began as a port of the wasm-split-prototype first developed
//! by @jbms at https://github.com/jbms/wasm-split-prototype
//! under the Apache License: https://github.com/jbms/wasm-split-prototype/blob/main/LICENSE

mod dep_graph;
mod emit;
mod read;
mod split_point;

use crate::{config::Project, internal_prelude::*, wasm_split_tools::dep_graph::DepNode};
use camino::Utf8PathBuf;
use split_point::SplitModuleIdentifier;
use std::collections::{HashMap, HashSet};

pub async fn wasm_split(
    input_wasm: &[u8],
    verbose: bool,
    proj: &Project,
) -> Result<Vec<Utf8PathBuf>> {
    let mut split_files = vec![];

    let module = self::read::InputModule::parse(input_wasm)?;

    let wb_descriptors = module
        .names
        .functions
        .iter()
        .filter_map(|(id, name)| is_wasm_bindgen_descriptor(name).then_some(DepNode::Function(*id)))
        .collect::<HashSet<_>>();

    let dep_graph = dep_graph::get_dependencies(&module)?;
    let split_points = split_point::get_split_points(&module)?;
    let mut split_program_info =
        split_point::compute_split_modules(&module, &dep_graph, &split_points, &wb_descriptors)?;

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
        |identifier: &SplitModuleIdentifier, data: &[u8]| -> Result<()> {
            let output_path = if matches!(identifier, SplitModuleIdentifier::Main) {
                let mut source = proj.lib.wasm_file.source.clone();
                source.set_file_name(format!("{}_split.wasm", source.file_stem().unwrap()));
                source
            } else {
                dest_dir.join(format!("{}.wasm", identifier.name_hashed(proj)))
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
    let mut manifest = HashMap::<String, Vec<String>>::new();
    for (name, _) in split_program_info.output_modules.iter() {
        let SplitModuleIdentifier::Chunk { splits, .. } = name else {
            continue;
        };
        let chunk_name = name.name(proj);
        let chunk_name_hashed = name.name_hashed(proj);

        manifest
            .entry(chunk_name.clone())
            .or_default()
            .push(chunk_name_hashed.clone());

        for split in splits {
            split_deps
                .entry(split.clone())
                .or_default()
                .push(chunk_name.clone());
            manifest
                .entry(split.clone())
                .or_default()
                .push(chunk_name_hashed.clone());
        }

        javascript.push_str(
            format!(
                "const __wasm_split_load_{chunk_name} = makeLoad(new URL(\"./{chunk_name_hashed}.wasm\", import.meta.url), []);\n",
            ).as_str()
        );
    }
    for (identifier, _) in split_program_info.output_modules.iter().rev() {
        let SplitModuleIdentifier::Split { name, .. } = identifier else {
            continue;
        };
        let name_hashed = identifier.name_hashed(proj);

        manifest
            .entry(identifier.name(proj))
            .or_default()
            .push(name_hashed.clone());

        javascript.push_str(format!(
            "export const __wasm_split_load_{name} = makeLoad(new URL(\"./{name_hashed}.wasm\", import.meta.url), [{deps}]);\n",
            deps = split_deps
            .remove(name)
            .unwrap_or_default()
            .iter()
            .map(|x| format!("__wasm_split_load_{x}"))
            .collect::<Vec<_>>()
            .join(", "),
        ).as_str());
    }

    tokio::fs::write(
        proj.lib
            .wasm_file
            .dest
            .parent()
            .expect("no destination directory")
            .join("__wasm_split_manifest.json"),
        serde_json::to_string_pretty(&manifest).expect("could not serialize manifest file"),
    )
    .await?;

    tokio::fs::write(
        proj.lib
            .wasm_file
            .dest
            .parent()
            .expect("no destination directory")
            .join("__wasm_split.______________________.js"),
        javascript,
    )
    .await?;

    Ok(split_files)
}

fn is_wasm_bindgen_descriptor(name: &str) -> bool {
    name == "__wbindgen_describe_closure" || name == "__wbindgen_describe"
}
