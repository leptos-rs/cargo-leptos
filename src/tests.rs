use camino::Utf8PathBuf;

use crate::{
    config::{Cli, Commands, Opts},
    ext::PathBufExt,
    run,
};

#[tokio::test]
async fn workspace_build() {
    let command = Commands::Build(Opts::default());

    let cli = Cli {
        manifest_path: Some(Utf8PathBuf::from("examples/workspace/Cargo.toml")),
        log: Vec::new(),
        command,
    };

    run(cli).await.unwrap();

    // when running the current working directory is changed to the manifest path.
    let site_dir = Utf8PathBuf::from("target/site");

    insta::assert_display_snapshot!(site_dir.ls_ascii(0).unwrap_or_default());
}

// TODO: `cargo-leptos` sets the cwd which is a global env
// and that prevents builds to run in parallel in the same process
//
// #[tokio::test]
// async fn project_build() {
//     let command = Commands::Build(Opts::default());

//     let cli = Cli {
//         manifest_path: Some(Utf8PathBuf::from("examples/project/Cargo.toml")),
//         log: Vec::new(),
//         command,
//     };

//     run(cli).await.unwrap();

//     // when running the current working directory is changed to the manifest path.
//     let site_dir = Utf8PathBuf::from("target/site");

//     insta::assert_display_snapshot!(site_dir.ls_ascii(0).unwrap_or_default());
// }
