//! This module began as a port of the wasm-split-prototype first developed
//! by @jbms at https://github.com/jbms/wasm-split-prototype
//! under the Apache License: https://github.com/jbms/wasm-split-prototype/blob/main/LICENSE

mod dep_graph;
mod emit;
mod read;
mod split_point;

use crate::{config::Project, internal_prelude::*};
use camino::{Utf8Path, Utf8PathBuf};
use split_point::SplitModuleIdentifier;
use std::collections::HashMap;

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

    if true {
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

    std::fs::create_dir_all(dest_dir)?;

    self::emit::emit_modules(
        &module,
        &mut split_program_info,
        |identifier: &SplitModuleIdentifier, data: &[u8]| -> Result<()> {
            let output_path = if matches!(identifier, SplitModuleIdentifier::Main) {
                main_out_file.clone()
            } else {
                dest_dir.join(format!("{}.wasm", identifier.hash()))
            };

            std::fs::write(&output_path, data)?;

            if !matches!(identifier, SplitModuleIdentifier::Main) {
                split_files.push(output_path);
            }

            Ok(())
        },
    )?;

    let mut javascript = String::new();
    javascript.push_str(r#"import { initSync } from ""#);
    javascript.push_str(main_module);
    javascript.push_str(
        r#"";
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
    for (identifier, _) in split_program_info.output_modules.iter() {
        let SplitModuleIdentifier::Chunk { splits, hash } = identifier else {
            continue;
        };
        let name = identifier.name();

        manifest.entry(name.clone()).or_default().push(hash.clone());

        for split in splits {
            split_deps
                .entry(split.clone())
                .or_default()
                .push(name.clone());
            manifest
                .entry(split.clone())
                .or_default()
                .push(hash.clone());
        }

        javascript.push_str(
            format!(
                "const __wasm_split_load_{name} = makeLoad(new URL(\"./{hash}.wasm\", import.meta.url), []);\n",
            ).as_str()
        );
    }
    for (identifier, _) in split_program_info.output_modules.iter().rev() {
        let SplitModuleIdentifier::Split { name, hash } = identifier else {
            continue;
        };

        manifest
            .entry(identifier.name())
            .or_default()
            .push(hash.clone());

        javascript.push_str(format!(
            "export const __wasm_split_load_{name} = makeLoad(new URL(\"./{hash}.wasm\", import.meta.url), [{deps}]);\n",
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
        dest_dir.join("__wasm_split_manifest.json"),
        serde_json::to_string_pretty(&manifest).expect("could not serialize manifest file"),
    )
    .await?;

    tokio::fs::write(
        dest_dir.join("__wasm_split.______________________.js"),
        javascript,
    )
    .await?;

    Ok(split_files)
}
