use super::build::build_proj;
use crate::internal_prelude::*;
use crate::{
    compile::{self},
    config::Project,
    service,
    signal::{Interrupt, Outcome, Product, ProductSet, ReloadSignal, ServerRestart},
};
use leptos_hot_reload::ViewMacros;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;
use tokio::try_join;

pub async fn watch(proj: &Arc<Project>) -> Result<()> {
    // even if the build fails, we continue
    build_proj(proj).await?;

    // but if ctrl-c is pressed, we stop
    if Interrupt::is_shutdown_requested().await {
        return Ok(());
    }

    if proj.hot_reload && proj.release {
        log::warn!("warning: Hot reloading does not currently work in --release mode.");
    }

    let view_macros = if proj.hot_reload {
        // build initial set of view macros for patching
        let view_macros = ViewMacros::new();
        view_macros
            .update_from_paths(&proj.lib.src_paths)
            .wrap_anyhow_err("Couldn't update view-macro watch")?;
        Some(view_macros)
    } else {
        None
    };

    service::notify::spawn(proj, view_macros).await?;
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
        debug!("Watch waiting for changes");

        let int = int.recv().await;
        // Do not terminate the execution of watch if the receiver lagged behind as it might be a slow receiver
        // It happens when many files are modified in short period and it exceeds the channel capacity.
        if matches!(int, Err(RecvError::Closed)) {
            return Err(RecvError::Closed).dot();
        }

        if Interrupt::is_shutdown_requested().await {
            debug!("Shutting down");
            return Ok(());
        }

        runner(proj).await?;
    }
}

pub async fn runner(proj: &Arc<Project>) -> Result<()> {
    let changes = Interrupt::get_source_changes().await;

    let server_hdl = compile::server(proj, &changes).await;
    let front_hdl = compile::front(proj, &changes).await;
    let assets_hdl = compile::assets(proj, &changes).await;
    let style_hdl = compile::style(proj, &changes).await;

    let (server, front, assets, style) = try_join!(server_hdl, front_hdl, assets_hdl, style_hdl)?;

    let build_scripts = compile::run_build_scripts(proj).await.await?;

    let outcomes = vec![server?, front?, assets?, style?, build_scripts?];

    let interrupted = outcomes.contains(&Outcome::Stopped);
    if interrupted {
        info!("Build interrupted. Restarting.");
        return Ok(());
    }

    let failed = outcomes.contains(&Outcome::Failed);
    if failed {
        warn!("Build failed");
        Interrupt::clear_source_changes().await;
        return Ok(());
    }

    let set = ProductSet::from(outcomes);

    if set.is_empty() {
        trace!("Build step done with no changes");
    } else {
        trace!("Build step done with changes: {set}");
    }

    if set.contains(&Product::Server) {
        // send product change, then the server will send the reload once it has restarted
        ServerRestart::send();
        info!("Watch updated {set}. Server restarting")
    } else if set.only_style() {
        ReloadSignal::send_style();
        info!("Watch updated style")
    } else if set.contains_any(&[Product::Front, Product::Assets]) {
        ReloadSignal::send_full();
        info!("Watch updated {set}")
    }
    Interrupt::clear_source_changes().await;
    Ok(())
}
