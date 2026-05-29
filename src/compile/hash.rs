use crate::{
    config::Project,
    ext::eyre::CustomWrapErr,
    internal_prelude::*,
};
use base64ct::{Base64UrlUnpadded, Encoding};
use camino::Utf8PathBuf;
use eyre::{ContextCompat, Result};
use md5::{Digest, Md5};
use std::{collections::HashMap, fs};

///Adds hashes to the filenames of the css, js, and wasm files in the output
pub fn add_hashes_to_site(proj: &Project) -> Result<()> {
    let mut files_to_hashes = compute_front_file_hashes(proj).dot()?;
    let pkg_dir = proj.site.root_relative_pkg_dir();

    let old_wasm_split = pkg_dir.join(crate::wasm_split_tools::WASM_SPLIT_LOADER_PLACEHOLDER);

    if proj.split {
        // Finalized separately: its hash depends on the chunk names rename_files produces.
        files_to_hashes.remove(&old_wasm_split);
    }

    debug!("Hash computed: {files_to_hashes:?}");

    let mut renamed_files = rename_files(&files_to_hashes).dot()?;

    let wasm_split_hash = if proj.split {
        let (final_wasm_split, final_hash) =
            finalize_wasm_split_loader(&old_wasm_split, &renamed_files, &pkg_dir)?;
        renamed_files.insert(old_wasm_split.clone(), final_wasm_split.clone());

        let old_wasm_split_filename = old_wasm_split.file_name().unwrap();
        let final_wasm_split_filename = final_wasm_split.file_name().unwrap();

        for entry in fs::read_dir(&pkg_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                    if filename.ends_with(".wasm") {
                        replace_in_binary_file(
                            &Utf8PathBuf::try_from(path).unwrap(),
                            old_wasm_split_filename,
                            final_wasm_split_filename,
                        );
                    } else if filename.starts_with("__wasm_split_manifest") {
                        replace_in_file(
                            &Utf8PathBuf::try_from(path).unwrap(),
                            &renamed_files,
                            &pkg_dir,
                            true,
                        );
                    } else if filename.starts_with("__wasm_split") {
                        replace_in_file(
                            &Utf8PathBuf::try_from(path).unwrap(),
                            &renamed_files,
                            &pkg_dir,
                            false,
                        );
                    }
                }
            }
        }

        Some(final_hash)
    } else {
        None
    };

    // Rewritten last, so renamed_files already holds the loader's final name.
    replace_in_file(
        &renamed_files[&proj.lib.js_file.dest],
        &renamed_files,
        &pkg_dir,
        false,
    );

    let manifest_file = files_to_hashes
        .iter()
        .find_map(|(f, h)| (f.ends_with("__wasm_split_manifest.json")).then_some(h));
    let manifest_file = manifest_file
        .map(|f| format!("manifest: {f}\n"))
        .unwrap_or_default();
    let wasm_split_file = wasm_split_hash
        .map(|f| format!("split: {f}\n"))
        .unwrap_or_default();

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
            "{}: {}\n{}: {}\n{}: {}\n{}{}",
            proj.lib
                .js_file
                .dest
                .extension()
                .ok_or(eyre!("no extension"))?,
            files_to_hashes[&proj.lib.js_file.dest],
            proj.lib
                .wasm_file
                .dest
                .extension()
                .ok_or(eyre!("no extension"))?,
            files_to_hashes[&proj.lib.wasm_file.dest],
            proj.style
                .site_file
                .dest
                .extension()
                .ok_or(eyre!("no extension"))?,
            files_to_hashes[&proj.style.site_file.dest],
            manifest_file,
            wasm_split_file
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

                    // We do not want to hash files generated by webassembly
                    // in the snippets folder as the webassembly will look for
                    // unhashed versions of the .js files. The folder though can be hashed.
                    if let Some(path_str) = path.to_str() {
                        if path_str.contains("snippets") && path.is_file(){
                            continue;
                        }
                    }

                    let hash = content_hash(&path)?;

                    if path
                        .file_stem()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name == hash)
                    {
                        continue;
                    }

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

/// Content hash used for cache-busting filenames (base64url-unpadded MD5).
fn content_hash(path: impl AsRef<std::path::Path>) -> Result<String> {
    Ok(Base64UrlUnpadded::encode_string(
        &Md5::new().chain_update(fs::read(path)?).finalize(),
    ))
}

fn rename_files(
    files_to_hashes: &HashMap<Utf8PathBuf, String>,
) -> Result<HashMap<Utf8PathBuf, Utf8PathBuf>> {
    const HASH_PLACEHOLDER: &str = "______________________";
    let mut old_to_new_paths = HashMap::new();

    for (path, hash) in files_to_hashes {
        let mut new_path = path.clone();

        let file_name = new_path.file_name().unwrap_or_default();

        let new_file_name = if file_name.contains(HASH_PLACEHOLDER) {
            if hash.len() != HASH_PLACEHOLDER.len() {
                return Err(anyhow!(
                    "File hash length did not match placeholder hash length."
                ));
            }
            file_name.replace(HASH_PLACEHOLDER, hash)
        } else {
            format!(
                "{}.{}.{}",
                path.file_stem().ok_or(eyre!("no file stem"))?,
                hash,
                path.extension().ok_or(eyre!("no extension"))?,
            )
        };

        new_path.set_file_name(new_file_name);

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
    omit_extension: bool,
) {
    let mut contents = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("error {e}: could not read file {path}"));

    for (old_path, new_path) in old_to_new_paths {
        let old_path = old_path
            .strip_prefix(root_dir)
            .expect("could not strip root path");
        let new_path = new_path
            .strip_prefix(root_dir)
            .expect("could not strip root path");

        if omit_extension {
            let old_path = old_path.as_str().trim_end_matches(".wasm");
            let new_path = new_path.as_str().trim_end_matches(".wasm");
            contents = contents.replace(old_path, new_path);
        } else {
            contents = contents.replace(old_path.as_str(), new_path.as_str());
        }
    }

    fs::write(path, contents).expect("could not write file");
}

// Rewrites the loader's chunk references, then hashes and renames it to its
// content-addressed name. Returns (final_path, final_hash).
fn finalize_wasm_split_loader(
    old_path: &Utf8PathBuf,
    renamed_files: &HashMap<Utf8PathBuf, Utf8PathBuf>,
    pkg_dir: &Utf8PathBuf,
) -> Result<(Utf8PathBuf, String)> {
    replace_in_file(old_path, renamed_files, pkg_dir, false);
    let hash = content_hash(old_path)?;
    let renamed = rename_files(&HashMap::from([(old_path.clone(), hash.clone())]))?;
    Ok((renamed[old_path].clone(), hash))
}

fn replace_in_binary_file(path: &Utf8PathBuf, old_wasm_split: &str, new_wasm_split: &str) {
    let mut contents =
        fs::read(path).unwrap_or_else(|e| panic!("error {e}: could not read file {path}"));

    let old_path = old_wasm_split.as_bytes();
    let new_path = new_wasm_split.as_bytes();

    for i in 0..=contents.len() - old_path.len() {
        if contents[i..].starts_with(old_path) {
            contents[i..(i + old_path.len())].clone_from_slice(new_path);
        }
    }

    fs::write(path, contents).expect("could not write file");
}

#[cfg(test)]
mod tests {
    use super::*;
    use temp_dir::TempDir;

    fn run_in_dir(pkg_dir: &Utf8PathBuf, chunk0: &[u8], chunk1: &[u8]) -> String {
        let loader = b"import('./chunk_0.wasm');\nimport('./chunk_1.wasm');\n";
        fs::write(pkg_dir.join("chunk_0.wasm"), chunk0).unwrap();
        fs::write(pkg_dir.join("chunk_1.wasm"), chunk1).unwrap();
        let old_wasm_split = pkg_dir.join(crate::wasm_split_tools::WASM_SPLIT_LOADER_PLACEHOLDER);
        fs::write(&old_wasm_split, loader).unwrap();

        // Only the chunks go in the map; finalize_wasm_split_loader hashes the
        // loader itself, after its chunk references have been rewritten.
        let mut files_to_hashes = HashMap::new();
        for name in ["chunk_0.wasm", "chunk_1.wasm"] {
            let path = pkg_dir.join(name);
            let hash = content_hash(&path).unwrap();
            files_to_hashes.insert(path, hash);
        }

        let renamed = rename_files(&files_to_hashes).unwrap();
        let (final_path, _) =
            finalize_wasm_split_loader(&old_wasm_split, &renamed, pkg_dir).unwrap();
        final_path.file_name().unwrap().to_string()
    }

    #[test]
    fn wasm_split_loader_filename_reflects_post_rewrite_content() {
        // Two builds share identical pre-rewrite loader content but differ in
        // chunk content, so after chunk hashes are substituted into the loader
        // the two loaders differ and must therefore have different filenames.
        let dir_a = TempDir::new().unwrap();
        let pkg_a = Utf8PathBuf::from_path_buf(dir_a.path().to_path_buf()).unwrap();

        let dir_b = TempDir::new().unwrap();
        let pkg_b = Utf8PathBuf::from_path_buf(dir_b.path().to_path_buf()).unwrap();

        let name_a = run_in_dir(&pkg_a, b"chunk0_build_a", b"chunk1_build_a");
        let name_b = run_in_dir(&pkg_b, b"chunk0_build_b", b"chunk1_build_b");

        assert_ne!(
            name_a, name_b,
            "builds with different chunk content must produce different wasm_split loader \
             filenames; got {name_a} for both"
        );
    }
}
