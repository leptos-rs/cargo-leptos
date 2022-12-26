use super::exe::Exe;
use crate::ext::path::PathBufExt;
use camino::Utf8PathBuf;
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
    Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
        .unwrap()
        .ls_ascii()
        .unwrap_or_default()
}
