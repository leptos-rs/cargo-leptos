use camino::Utf8PathBuf;

use crate::{
    ext::{fs, PathBufExt},
    run, Cli, Commands, Opts,
};

#[tokio::test]
async fn workspace_build() {
    let command = Commands::Build(Opts::default());

    let cli = Cli {
        manifest_path: Some(Utf8PathBuf::from("examples/workspace/Cargo.toml")),
        log: Vec::new(),
        command,
    };

    if Utf8PathBuf::from("examples/workspace/target").exists() {
        fs::rm_dir_content("examples/workspace/target")
            .await
            .unwrap();
    }

    run(cli).await.unwrap();

    // when running the current working directory is changed to the manifest path.
    let site_dir = Utf8PathBuf::from("target/site");

    insta::assert_display_snapshot!(site_dir.ls_ascii(0).unwrap_or_default());
}
