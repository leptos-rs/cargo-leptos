use crate::config::Project;
use crate::ext::color_eyre::CustomWrapErr;
use crate::internal_prelude::*;
use base64ct::{Base64UrlUnpadded, Encoding};
use camino::Utf8PathBuf;
use color_eyre::eyre::ContextCompat;
use color_eyre::Result;
use md5::{Digest, Md5};
use std::collections::HashMap;
use std::fs;

///Adds hashes to the filenames of the css, js, and wasm files in the output
pub fn add_hashes_to_site(proj: &Project) -> Result<()> {
    let files_to_hashes = compute_front_file_hashes(proj).dot()?;

    debug!("Hash computed: {files_to_hashes:?}");

    let renamed_files = rename_files(&files_to_hashes).dot()?;

    // todo: maybe throw here if lib is empty? should rely on outer layers to get here with lib intact
    let lib = proj.lib.as_ref().unwrap();

    replace_in_file(
        &renamed_files[&lib.js_file.dest],
        &renamed_files,
        &proj.site.root_relative_pkg_dir(),
    );

    fs::create_dir_all(
        proj.hash_file
            .abs
            .parent()
            .wrap_err_with(|| format!("no parent dir for {}", proj.hash_file.abs))?,
    )
    .wrap_err_with(|| format!("Failed to create parent dir for {}", proj.hash_file.abs))?;

    fs::write(
        &proj.hash_file.abs,
        format!(
            "{}: {}\n{}: {}\n{}: {}\n",
            lib
                .js_file
                .dest
                .extension()
                .ok_or(eyre!("no extension"))?,
            files_to_hashes[&lib.js_file.dest],
            lib
                .wasm_file
                .dest
                .extension()
                .ok_or(eyre!("no extension"))?,
            files_to_hashes[&lib.wasm_file.dest],
            proj.style
                .site_file
                .dest
                .extension()
                .ok_or(eyre!("no extension"))?,
            files_to_hashes[&proj.style.site_file.dest]
        ),
    )
    .wrap_err_with(|| format!("Failed to write hash file to {}", proj.hash_file.abs))?;

    debug!("Hash written to {}", proj.hash_file.abs);

    Ok(())
}

fn compute_front_file_hashes(proj: &Project) -> Result<HashMap<Utf8PathBuf, String>> {
    let mut files_to_hashes = HashMap::new();

    let mut stack = vec![proj.site.root_relative_pkg_dir().into_std_path_buf()];

    while let Some(path) = stack.pop() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.is_file() {
                    if let Some(extension) = path.extension() {
                        if extension == "css" && path != proj.style.site_file.dest {
                            continue;
                        }
                    }

                    // Check if the path contains snippets and also if it
                    // contains inline{}.js. We do not want to hash these files
                    // as the webassembly will look for an unhashed version of
                    // the .js file. The folder though can be hashed.
                    if let Some(path_str) = path.to_str() {
                        if path_str.contains("snippets") {
                            if let Some(file_name) = path.file_name() {
                                let file_name_str = file_name.to_string_lossy();
                                if file_name_str.contains("inline") {
                                    if let Some(extension) = path.extension() {
                                        if extension == "js" {
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    let hash = Base64UrlUnpadded::encode_string(
                        &Md5::new().chain_update(fs::read(&path)?).finalize(),
                    );

                    files_to_hashes.insert(
                        Utf8PathBuf::from_path_buf(path).expect("invalid path"),
                        hash,
                    );
                } else if path.is_dir() {
                    stack.push(path);
                }
            }
        }
    }

    Ok(files_to_hashes)
}

fn rename_files(
    files_to_hashes: &HashMap<Utf8PathBuf, String>,
) -> Result<HashMap<Utf8PathBuf, Utf8PathBuf>> {
    let mut old_to_new_paths = HashMap::new();

    for (path, hash) in files_to_hashes {
        let mut new_path = path.clone();

        new_path.set_file_name(format!(
            "{}.{}.{}",
            path.file_stem().ok_or(eyre!("no file stem"))?,
            hash,
            path.extension().ok_or(eyre!("no extension"))?,
        ));

        fs::rename(path, &new_path)
            .wrap_err_with(|| format!("Failed to rename {path} to {new_path}"))?;

        old_to_new_paths.insert(path.clone(), new_path);
    }

    Ok(old_to_new_paths)
}

fn replace_in_file(
    path: &Utf8PathBuf,
    old_to_new_paths: &HashMap<Utf8PathBuf, Utf8PathBuf>,
    root_dir: &Utf8PathBuf,
) {
    let mut contents = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("error {e}: could not read file {}", path));

    for (old_path, new_path) in old_to_new_paths {
        let old_path = old_path
            .strip_prefix(root_dir)
            .expect("could not strip root path");
        let new_path = new_path
            .strip_prefix(root_dir)
            .expect("could not strip root path");

        contents = contents.replace(old_path.as_str(), new_path.as_str());
    }

    fs::write(path, contents).expect("could not write file");
}
