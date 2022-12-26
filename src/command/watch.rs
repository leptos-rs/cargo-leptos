use std::sync::Arc;

use crate::{
    compile::{self},
    config::Project,
    ext::anyhow::Context,
    service,
    signal::{Interrupt, ProductChange, ProductSet, ReloadSignal},
};
use anyhow::Result;
use tokio::try_join;

pub async fn watch(proj: &Arc<Project>) -> Result<()> {
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
    let mut first_sync = true;
    loop {
        let changes = Interrupt::get_source_changes().await;

        let server_hdl = compile::server(proj, &changes).await;
        let front_hdl = compile::front(proj, &changes).await;
        let assets_hdl = compile::assets(proj, &changes, first_sync).await;
        let style_hdl = compile::style(proj, &changes).await;

        let (serve, front, assets, style) =
            try_join!(server_hdl, front_hdl, assets_hdl, style_hdl)?;

        let set = ProductSet::from(vec![serve?, front?, assets?, style?]);

        log::trace!("Build step done with changes: {set}");
        first_sync = false;
        if set.only_style() {
            ReloadSignal::send_style();
            log::info!("Watch updated style")
        } else if !set.is_empty() {
            ProductChange::send(set.clone());
        }
        Interrupt::clear_source_changes().await;

        log::debug!("Watch waiting for changes");
        int.recv().await.dot()?;
        log::debug!("Watch Changes in {set}");

        if Interrupt::is_shutdown_requested().await {
            log::debug!("Shutting down");
            return Ok(());
        } else {
            log::debug!("Watch build output didn't change")
        }
    }
}
