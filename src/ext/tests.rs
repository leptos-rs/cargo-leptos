use super::exe::Exe;
use crate::ext::path::PathBufExt;
use camino::Utf8PathBuf;
use temp_dir::TempDir;

#[tokio::test]
async fn download_sass() {
    let dir = TempDir::new().unwrap();
    let meta = Exe::Sass.meta().await.unwrap();
    let e = meta.with_cache_dir(dir.path()).await;

    assert!(e.is_ok(), "{e:#?}\n{:#?}\nFiles: \n {}", meta, ls(&dir));

    let e = e.unwrap();
    assert!(e.exists(), "{:#?}\nFiles: \n{}", meta, ls(&dir));
}

#[tokio::test]
async fn download_tailwind() {
    let dir = TempDir::new().unwrap();
    let meta = Exe::Tailwind.meta().await.unwrap();
    let e = meta.with_cache_dir(dir.path()).await;
    assert!(e.is_ok(), "{e:#?}\n{:#?}\nFiles: \n {}", meta, ls(&dir));

    let e = e.unwrap();
    assert!(e.exists(), "{:#?}\nFiles: \n{}", meta, ls(&dir))
}

#[tokio::test]
async fn download_cargo_generate() {
    let dir = TempDir::new().unwrap();
    let meta = Exe::CargoGenerate.meta().await.unwrap();
    let e = meta.with_cache_dir(dir.path()).await;

    assert!(e.is_ok(), "{e:#?}\n{:#?}\nFiles: \n {}", meta, ls(&dir));

    let e = e.unwrap();
    assert!(e.exists(), "{:#?}\nFiles: \n{}", meta, ls(&dir));
}

#[tokio::test]
async fn download_wasmopt() {
    let dir = TempDir::new().unwrap();
    let meta = Exe::WasmOpt.meta().await.unwrap();
    let e = meta.with_cache_dir(dir.path()).await;

    assert!(e.is_ok(), "{e:#?}\n{:#?}\nFiles: \n {}", meta, ls(&dir));

    let e = e.unwrap();
    assert!(e.exists(), "{:#?}\nFiles: \n{}", meta, ls(&dir));
}

fn ls(dir: &TempDir) -> String {
    Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
        .unwrap()
        .ls_ascii(0)
        .unwrap_or_default()
}
