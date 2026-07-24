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
use tokio::join;
use tokio::sync::broadcast::error::RecvError;
use tokio::task::JoinHandle;

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
    let serve_service_jh = service::serve::spawn(proj).await;
    let reload_service_jh = service::reload::spawn(proj).await;

    let res = run_loop(proj, serve_service_jh, reload_service_jh).await;
    if res.is_err() {
        Interrupt::request_shutdown().await;
    }
    res
}

pub async fn run_loop(
    proj: &Arc<Project>,
    serve_service_jh: JoinHandle<Result<()>>,
    reload_service_jh: JoinHandle<()>,
) -> Result<()> {
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
            // Block until both services have finished their own shutdown work before returning.
            // This is important for graceful termination to work. The timeout-driven termination
            // inside `service::serve::ServerProcess::terminate` only has time to run if we keep the
            // runtime alive while it does. Returning here early, without awaiting, would drop the
            // tokio runtime out from under our in-flight `terminate()` call and the reload server's
            // shutdown branch, defeating the graceful path entirely.
            debug!("Shutting down. Waiting for serve and reload services to finish.");
            let (serve_join, reload_join) = join!(serve_service_jh, reload_service_jh);
            match serve_join {
                Ok(Ok(())) => {}
                Ok(Err(err)) => error!("'serve' service shut down with error: {err}"),
                Err(err) => error!("Error while waiting for 'serve' service to shut down: {err}"),
            }
            if let Err(err) = reload_join {
                error!("Error while waiting for 'reload' service to shut down: {err}");
            }
            return Ok(());
        }

        runner(proj).await?;
    }
}

pub async fn runner(proj: &Arc<Project>) -> Result<()> {
    let changes = Interrupt::get_source_changes().await;

    // if there were sourcecode changes and the clear cli option is set we clear the terminal
    if !changes.is_empty() && proj.clear_terminal_on_rebuild {
        clearscreen::clear()?;
    }

    // Honor `--frontend-only` / `--server-only` on rebuilds the same way the
    // initial `build_proj` does, so the flags apply uniformly (see issue #670).
    let needs_frontend = !proj.build_server_only;
    let needs_server = !proj.build_frontend_only;

    // Spawn all enabled compiles up front so they run concurrently, then join them.
    let mut server_hdl = None;
    if needs_server {
        server_hdl = Some(compile::server(proj, &changes).await);
    }
    let mut front_hdl = None;
    let mut assets_hdl = None;
    let mut style_hdl = None;
    if needs_frontend {
        front_hdl = Some(compile::front(proj, &changes).await);
        assets_hdl = Some(compile::assets(proj, &changes).await);
        style_hdl = Some(compile::style(proj, &changes).await);
    }

    let mut outcomes = Vec::new();
    if let Some(hdl) = server_hdl {
        outcomes.push(hdl.await??);
    }
    if let Some(hdl) = front_hdl {
        outcomes.push(hdl.await??);
    }
    if let Some(hdl) = assets_hdl {
        outcomes.push(hdl.await??);
    }
    if let Some(hdl) = style_hdl {
        outcomes.push(hdl.await??);
    }

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
