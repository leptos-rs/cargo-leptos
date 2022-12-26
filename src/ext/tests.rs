use super::exe::Exe;
use crate::ext::anyhow::Result;
use camino::Utf8PathBuf;
use std::collections::VecDeque;
use std::path::Path;
use temp_dir::TempDir;

#[tokio::test]
async fn download_sass() {
    let dir = TempDir::new().unwrap();
    let meta = Exe::Sass.meta_with_dir(dir.path().to_path_buf()).unwrap();
    let e = meta.from_cache().await;
    assert!(e.is_ok(), "{e:#?}\n{:#?}\nFiles: \n {}", meta, ls(&dir));

    let e = e.unwrap();
    assert!(e.exists(), "{:#?}\nFiles: \n{}", meta, ls(&dir));
}

#[tokio::test]
async fn download_cargo_generate() {
    let dir = TempDir::new().unwrap();
    let meta = Exe::CargoGenerate
        .meta_with_dir(dir.path().to_path_buf())
        .unwrap();

    let e = meta.from_cache().await;
    assert!(e.is_ok(), "{e:#?}\n{:#?}\nFiles: \n {}", meta, ls(&dir));

    let e = e.unwrap();
    assert!(e.exists(), "{:#?}\nFiles: \n{}", meta, ls(&dir));
}

#[tokio::test]
async fn download_wasmopt() {
    let dir = TempDir::new().unwrap();
    let meta = Exe::WasmOpt
        .meta_with_dir(dir.path().to_path_buf())
        .unwrap();
    let e = meta.from_cache().await;
    assert!(e.is_ok(), "{e:#?}\n{:#?}\nFiles: \n {}", meta, ls(&dir));

    let e = e.unwrap();
    assert!(e.exists(), "{:#?}\nFiles: \n{}", meta, ls(&dir));
}

fn ls(dir: &TempDir) -> String {
    ls_path(&dir.path()).unwrap_or_default()
}

fn ls_path(root: &Path) -> Result<String> {
    let mut dirs: VecDeque<(usize, Utf8PathBuf)> = VecDeque::new();
    let root = Utf8PathBuf::from_path_buf(root.to_path_buf()).unwrap();

    dirs.push_back((0, root));

    let mut out = Vec::new();

    while let Some((indent, dir)) = dirs.pop_front() {
        let mut entries = dir.read_dir_utf8()?;
        out.push(format!(
            "{}{}:",
            "  ".repeat(indent),
            dir.file_name().unwrap_or_default()
        ));

        let indent = indent + 1;
        while let Some(Ok(entry)) = entries.next() {
            let path = entry.path();

            if entry.file_type()?.is_dir() {
                dirs.push_back((indent, path.to_owned()));
            } else {
                out.push(format!(
                    "{}{}",
                    "  ".repeat(indent),
                    path.file_name().unwrap_or_default()
                ));
            }
        }
    }
    Ok(out.join("\n"))
}
