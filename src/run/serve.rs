use crate::config::Config;
use axum::{http::StatusCode, response::IntoResponse, routing::get_service, Router};
use std::{io, net::SocketAddr};
use tower_http::services::ServeDir;

pub async fn run(config: &Config) {
    let port = config.csr_port.unwrap_or(3001);
    let serve_dir = get_service(ServeDir::new("target/site")).handle_error(handle_error);

    let route = Router::new().nest("/", serve_dir.clone());

    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    axum::Server::bind(&addr)
        .serve(route.into_make_service())
        .await
        .unwrap();
}

async fn handle_error(_err: io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
}
