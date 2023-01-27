use std::sync::Arc;

use crate::{
    compile::{self},
    config::Project,
    ext::anyhow::Context,
    service,
    signal::{Interrupt, Outcome, Product, ProductSet, ReloadSignal, ServerRestart},
};
use anyhow::Result;
use tokio::try_join;

use super::build::build_proj;

pub async fn watch(proj: &Arc<Project>) -> Result<()> {
    build_proj(proj).await?;

    let _watch = service::notify::spawn(proj).await?;

    service::serve::spawn(proj).await;
    service::reload::spawn(proj).await;

    let res = run_loop(proj).await;
    if res.is_err() {
        Interrupt::request_shutdown().await;
    }
    res
}

pub async fn run_loop(proj: &Arc<Project>) -> Result<()> {
    let mut int = Interrupt::subscribe_any();
    loop {
        log::debug!("Watch waiting for changes");
        int.recv().await.dot()?;

        if Interrupt::is_shutdown_requested().await {
            log::debug!("Shutting down");
            return Ok(());
        }

        let changes = Interrupt::get_source_changes().await;

        let server_hdl = compile::server(proj, &changes).await;
        let front_hdl = compile::front(proj, &changes).await;
        let assets_hdl = compile::assets(proj, &changes, false).await;
        let style_hdl = compile::style(proj, &changes).await;

        let (serve, front, assets, style) =
            try_join!(server_hdl, front_hdl, assets_hdl, style_hdl)?;

        let outcomes = vec![serve?, front?, assets?, style?];
        let interrupted = outcomes.iter().any(|outcome| *outcome == Outcome::Stopped);
        if !interrupted {
            let set = ProductSet::from(outcomes);

            if set.is_empty() {
                log::trace!("Build step done with no changes");
            } else {
                log::trace!("Build step done with changes: {set}");
            }

            if set.only_style() {
                ReloadSignal::send_style();
                log::info!("Watch updated style")
            } else if set.contains(&Product::Server) {
                // send product change, then the server will send the reload once it has restarted
                ServerRestart::send();
                log::info!("Watch updated {set}. Server restarting")
            } else if set.contains_any(&[Product::Front, Product::Assets]) {
                ReloadSignal::send_full();
                log::info!("Watch updated {set}")
            }
            Interrupt::clear_source_changes().await;
        } else {
            log::info!("Watch interrupted. Restarting build step.");
        }
    }
}
