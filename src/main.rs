use cargo_leptos::{config::Cli, run};
use clap::Parser;
use std::env;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let mut args: Vec<String> = env::args().collect();
    // when running as cargo leptos, the second argument is "leptos" which
    // clap doesn't expect
    if args.get(1).map(|a| a == "leptos").unwrap_or(false) {
        args.remove(1);
    }

    let args = Cli::parse_from(&args);

    let verbose = args.opts().map(|o| o.verbose).unwrap_or(0);
    cargo_leptos::logger::setup(verbose, &args.log);
    color_eyre::install()?;

    run(args).await
}
