use super::build::build_proj;
use crate::internal_prelude::*;
use crate::{
    compile::{self},
    config::Project,
    service,
    signal::{Interrupt, Outcome, Product, ProductSet, ReloadSignal, ServerRestart},
};
use leptos_hot_reload::ViewMacros;
use tokio::try_join;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;

pub async fn watch(proj: &Arc<Project>) -> Result<()> {
    // even if the build fails, we continue
    build_proj(proj).await?;

    // but if ctrl-c is pressed, we stop
    if Interrupt::is_shutdown_requested().await {
        return Ok(());
    }

    if proj.hot_reload && proj.release {
        log::warn!("warning: Hot reloading does not currently work in --release mode.");
    } else if proj.hot_reload && proj.lib.is_none() {
        log::warn!("warning: Hot reloading won't do anything, running in bin-only mode");
    }

    let view_macros = if proj.hot_reload && proj.lib.is_some() {
        // build initial set of view macros for patching
        let view_macros = ViewMacros::new();
        view_macros
            .update_from_paths(&proj.lib.as_ref().unwrap().src_paths)
            .wrap_anyhow_err("Couldn't update view-macro watch")?;
        Some(view_macros)
    } else {
        None
    };

    service::notify::spawn(proj, view_macros).await?;
    if proj.bin.is_some() {
        service::serve::spawn(proj).await;
    }
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

    // todo: super hacky, gotta be a better way
    let server_hdl = if proj.bin.is_some() {
        compile::server(proj, &changes).await
    } else {
        tokio::spawn(async move { Ok(Outcome::Skipped) })
    };

    let (front_hdl, assets_hdl, style_hdl) = if proj.lib.is_some() {
        (
            compile::front(proj, &changes).await,
            compile::assets(proj, &changes).await,
            compile::style(proj, &changes).await
        )
    } else {
        (
            tokio::spawn(async move { Ok(Outcome::Skipped) }),
            tokio::spawn(async move { Ok(Outcome::Skipped) }),
            tokio::spawn(async move { Ok(Outcome::Skipped) })
        )
    };
    let (server, front, assets, style) = try_join!(server_hdl, front_hdl, assets_hdl, style_hdl)?;

    let outcomes = vec![server?, front?, assets?, style?];

    let interrupted = outcomes.iter().any(|outcome| *outcome == Outcome::Stopped);
    if interrupted {
        info!("Build interrupted. Restarting.");
        return Ok(());
    }

    let failed = outcomes.iter().any(|outcome| *outcome == Outcome::Failed);
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

    if set.contains(&Product::Server) && proj.bin.is_some() {
        // send product change, then the server will send the reload once it has restarted
        ServerRestart::send();
        info!("Watch updated {set}. Server restarting")
    } else if set.only_style() && proj.lib.is_some() {
        ReloadSignal::send_style();
        info!("Watch updated style")
    } else if set.contains_any(&[Product::Front, Product::Assets]) && proj.lib.is_some() {
        ReloadSignal::send_full();
        info!("Watch updated {set}")
    }
    Interrupt::clear_source_changes().await;
    Ok(())
}
