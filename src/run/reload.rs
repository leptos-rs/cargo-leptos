use crate::logger::GRAY;
use crate::util::oneshot_when;
use crate::MSG_BUS;
use crate::{config::Config, Msg};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use std::net::SocketAddr;
use tokio::task::JoinHandle;

pub async fn spawn(config: &Config) -> JoinHandle<()> {
    let route = Router::new().route("/ws", get(websocket_handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], config.leptos.reload_port));

    let shutdown_rx = oneshot_when(&[Msg::ShutDown], "Autoreload");

    log::debug!("Autoreload server started {}", GRAY.paint(addr.to_string()));

    tokio::spawn(async move {
        match axum::Server::bind(&addr)
            .serve(route.into_make_service())
            .with_graceful_shutdown(async move { drop(shutdown_rx.await.ok()) })
            .await
        {
            Ok(_) => log::debug!("Autoreload server stopped"),
            Err(e) => log::error!("Autoreload {e}"),
        }
    })
}

async fn websocket_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(websocket)
}

async fn websocket(mut stream: WebSocket) {
    let mut rx = MSG_BUS.subscribe();

    log::trace!("Autoreload websocket connected");
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(Msg::Reload(msg)) => match stream.send(Message::Text(msg.clone())).await {
                    Err(e) => {
                        log::debug!("Autoreload could not send {msg} due to {e}");
                        break;
                    }
                    Ok(_) => {
                        log::debug!("Autoreload sent \"{msg}\" to browser");
                        if let Err(e) = stream.close().await {
                            log::error!("Autoreload socket close error {e}");
                        }
                        break;
                    }
                },
                Err(e) => {
                    log::debug!("Autoreload recive error {e}");
                    break;
                }
                Ok(Msg::ShutDown) => break,
                _ => {}
            }
        }
        log::trace!("Autoreload websocket closed")
    });
}
