use camino::Utf8PathBuf;

use crate::{ext::path::PathBufExt, run, Cli, Commands, Opts};

#[tokio::test]
async fn workspace_build() {
    let command = Commands::Build(Opts::default());

    let cli = Cli {
        manifest_path: Some(Utf8PathBuf::from("examples/workspace/Cargo.toml")),
        log: Vec::new(),
        command,
    };

    run(cli).await.unwrap();

    let site_dir = Utf8PathBuf::from("target/site");

    insta::assert_display_snapshot!(site_dir.ls_ascii().unwrap_or_default());
}
