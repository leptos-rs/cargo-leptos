use crate::run::oneshot_when;
use crate::MSG_BUS;
use crate::{config::Config, Msg};
use anyhow::Result;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use std::net::SocketAddr;

pub async fn run(config: &Config) -> Result<()> {
    let route = Router::new().route("/ws", get(websocket_handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], config.reload_port));

    let shutdown_rx = oneshot_when(&[Msg::ShutDown], "reload shutdown");

    tokio::spawn(async move {
        match axum::Server::bind(&addr)
            .serve(route.into_make_service())
            .with_graceful_shutdown(async move {
                shutdown_rx.await.ok();
                log::debug!("autoreload server shutting down");
            })
            .await
        {
            Ok(_) => log::debug!("autoreload server shut down"),
            Err(e) => log::error!("autoreload error: {e}"),
        }
    });
    log::debug!("autoreload server started");
    Ok(())
}

async fn websocket_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(websocket)
}

async fn websocket(mut stream: WebSocket) {
    let mut rx = MSG_BUS.subscribe();

    log::debug!("autoreload websocket opened");
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(Msg::Reload(msg)) => {
                    if let Err(e) = stream.send(Message::Text(msg)).await {
                        log::debug!("autoreload: {e}");
                        break;
                    }
                }
                Err(e) => {
                    log::debug!("autoreload: {e}");
                    break;
                }
                _ => {}
            }
        }
    });
}
