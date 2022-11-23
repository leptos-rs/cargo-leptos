use crate::{
    config::Config,
    util::{oneshot_when, shutdown_msg},
};
use axum::{http::StatusCode, response::IntoResponse, routing::get_service, Router};
use std::{io, net::SocketAddr};
use tokio::task::JoinHandle;
use tower_http::services::ServeDir;

pub async fn spawn(config: &Config) -> JoinHandle<()> {
    let serve_dir = get_service(ServeDir::new("target/site")).handle_error(handle_error);

    let route = Router::new().nest("/", serve_dir.clone());

    let addr = SocketAddr::from(([127, 0, 0, 1], config.leptos.csr_port));

    let shutdown_rx = oneshot_when(shutdown_msg, "Server");

    log::info!("Serving client on {addr}");

    tokio::spawn(async move {
        match axum::Server::bind(&addr)
            .serve(route.into_make_service())
            .with_graceful_shutdown(async { drop(shutdown_rx.await.ok()) })
            .await
        {
            Ok(_) => log::debug!("Server stopped"),
            Err(e) => log::error!("Server {e}"),
        }
    })
}

async fn handle_error(_err: io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
}
