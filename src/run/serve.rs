use crate::{config::Config, util::oneshot_when, Msg};
use anyhow::Result;
use axum::{http::StatusCode, response::IntoResponse, routing::get_service, Router};
use std::{io, net::SocketAddr};
use tower_http::services::ServeDir;

pub async fn run(config: &Config) -> Result<()> {
    let serve_dir = get_service(ServeDir::new("target/site")).handle_error(handle_error);

    let route = Router::new().nest("/", serve_dir.clone());

    let addr = SocketAddr::from(([127, 0, 0, 1], config.leptos.csr_port));

    let shutdown_rx = oneshot_when(&[Msg::ShutDown], "Server");

    log::info!("Serving client on {addr}");
    if let Err(e) = axum::Server::bind(&addr)
        .serve(route.into_make_service())
        .with_graceful_shutdown(async {
            shutdown_rx.await.ok();
            log::debug!("Server stopped");
        })
        .await
    {
        log::error!("Server {e}");
    }
    Ok(())
}

async fn handle_error(_err: io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
}
