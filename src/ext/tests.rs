use super::exe::Exe;
use temp_dir::TempDir;

#[tokio::test]
async fn download_sass() {
    let dir = TempDir::new().unwrap();
    let meta = Exe::Sass.meta_with_dir(dir.path().to_path_buf()).unwrap();
    let e = meta.from_cache().await;
    assert!(e.is_ok(), "{e:#?}\n{:#?}", meta);

    let e = e.unwrap();
    assert!(e.exists(), "{:#?}", meta);
}

#[tokio::test]
async fn download_cargo_generate() {
    let dir = TempDir::new().unwrap();
    let meta = Exe::CargoGenerate
        .meta_with_dir(dir.path().to_path_buf())
        .unwrap();

    let e = meta.from_cache().await;
    assert!(e.is_ok(), "{e:#?}\n{:#?}", meta);

    let e = e.unwrap();
    assert!(e.exists(), "{:#?}", meta);
}

#[tokio::test]
async fn download_wasmopt() {
    let dir = TempDir::new().unwrap();
    let meta = Exe::WasmOpt
        .meta_with_dir(dir.path().to_path_buf())
        .unwrap();
    let e = meta.from_cache().await;
    assert!(e.is_ok(), "{e:#?}\n{:#?}", meta);

    let e = e.unwrap();
    assert!(e.exists(), "{:#?}", meta);
}
