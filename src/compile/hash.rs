use crate::config::Project;
use crate::ext::anyhow::Context;
use anyhow::Result;
use base64ct::{Base64UrlUnpadded, Encoding};
use camino::Utf8PathBuf;
use md5::{Digest, Md5};
use std::collections::HashMap;
use std::fs;

///Adds hashes to the filenames of the css, js, and wasm files in the output
pub fn add_hashes_to_site(proj: &Project) -> Result<()> {
    let files_to_hashes = compute_front_file_hashes(proj).dot()?;
    let renamed_files = rename_files(&files_to_hashes).dot()?;

    replace_in_file(
        &renamed_files[&proj.lib.js_file.dest],
        &renamed_files,
        &proj.site.root_relative_pkg_dir(),
    );

    fs::write(
        &proj.hash_file.abs,
        format!(
            "{}: {}\n{}: {}\n{}: {}\n",
            proj.lib
                .js_file
                .dest
                .extension()
                .ok_or(anyhow::anyhow!("no extension"))?,
            files_to_hashes[&proj.lib.js_file.dest],
            proj.lib
                .wasm_file
                .dest
                .extension()
                .ok_or(anyhow::anyhow!("no extension"))?,
            files_to_hashes[&proj.lib.wasm_file.dest],
            proj.style
                .site_file
                .dest
                .extension()
                .ok_or(anyhow::anyhow!("no extension"))?,
            files_to_hashes[&proj.style.site_file.dest]
        ),
    )?;

    Ok(())
}

fn compute_front_file_hashes(proj: &Project) -> Result<HashMap<Utf8PathBuf, String>> {
    let mut files_to_hashes = HashMap::new();

    let mut stack = vec![proj.site.root_relative_pkg_dir().into_std_path_buf()];

    while let Some(path) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {

                    let path = entry.path();

                    if path.is_file() {
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
            path.file_stem().ok_or(anyhow::anyhow!("no file stem"))?,
            hash,
            path.extension().ok_or(anyhow::anyhow!("no extension"))?,
        ));

        fs::rename(path, &new_path)?;

        old_to_new_paths.insert(path.clone(), new_path);
    }

    Ok(old_to_new_paths)
}

fn replace_in_file(
    path: &Utf8PathBuf,
    old_to_new_paths: &HashMap<Utf8PathBuf, Utf8PathBuf>,
    root_dir: &Utf8PathBuf,
) {
    let mut contents = fs::read_to_string(path).expect("could not read file");

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
