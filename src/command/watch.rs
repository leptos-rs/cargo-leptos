use crate::{
    command::{
        clear_source_changes, get_source_changes, is_shutdown_requested, send_product_change,
        send_reload, ReloadType,
    },
    config::Config,
    ext::anyhow::Context,
    service,
    task::compile::{self, ProductSet},
};
use anyhow::Result;
use tokio::{task::JoinHandle, try_join};

use super::{ctrl_c_monitor, request_shutdown, subscribe_interrupt};

pub async fn run(conf: &Config) -> Result<()> {
    let _ = ctrl_c_monitor();
    let _ = watch_changes(&conf).await?;

    service::serve::run(conf).await;
    service::reload::run(conf).await;

    let res = run_loop(conf).await;
    if res.is_err() {
        request_shutdown().await;
    }
    res
}

pub async fn run_loop(conf: &Config) -> Result<()> {
    let mut int = subscribe_interrupt();
    let mut first_sync = true;
    loop {
        let changes = get_source_changes().await;

        let server_hdl = compile::server(conf, &changes).await;
        let front_hdl = compile::front(conf, &changes).await;
        let assets_hdl = compile::assets(conf, &changes, first_sync).await;
        let style_hdl = compile::style(conf, &changes).await;

        let (serve, front, assets, style) =
            try_join!(server_hdl, front_hdl, assets_hdl, style_hdl)?;

        let set = ProductSet::from(vec![serve?, front?, assets?, style?]);

        log::trace!("Build step done with changes: {set}");
        first_sync = false;
        if set.only_style() {
            send_reload(ReloadType::Style);
            log::info!("Watch updated style")
        } else if !set.is_empty() {
            send_product_change(set.clone());
        }
        clear_source_changes().await;

        log::debug!("Watch waiting for changes");
        int.recv().await.dot()?;
        log::debug!("Watch Changes in {set}");

        if is_shutdown_requested().await {
            log::debug!("Shutting down");
            return Ok(());
        } else {
            log::debug!("Watch build output didn't change")
        }
    }
}

async fn watch_changes(conf: &Config) -> Result<JoinHandle<()>> {
    service::notify::spawn(conf).await
}
