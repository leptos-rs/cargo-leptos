use crate::ext::anyhow::{bail, Result};
use crate::logger::{BOLD, GRAY};
use crate::sync::{oneshot_when, shutdown_msg, wait_for_localhost};
use crate::{Config, Msg, MSG_BUS};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use std::net::SocketAddr;
use tokio::{net::TcpStream, task::JoinHandle};

pub async fn spawn(config: &Config) -> Result<JoinHandle<()>> {
    let port = config.leptos.csr_port;
    let route = Router::new().route("/autoreload", get(move |ws| websocket_handler(ws, port)));

    let addr = SocketAddr::from(([127, 0, 0, 1], config.leptos.reload_port));

    if let Ok(_) = TcpStream::connect(&addr).await {
        bail!(
            "Server port {} already in use. You can set which port to use with {} in {} section {}",
            config.leptos.reload_port,
            BOLD.paint("reload_port"),
            BOLD.paint("Cargo.toml"),
            BOLD.paint("[package.metadata.leptos]"),
        );
    }

    let shutdown_rx = oneshot_when(shutdown_msg, "Autoreload");

    log::debug!("Autoreload server started {}", GRAY.paint(addr.to_string()));

    Ok(tokio::spawn(async move {
        match axum::Server::bind(&addr)
            .serve(route.into_make_service())
            .with_graceful_shutdown(async move { drop(shutdown_rx.await.ok()) })
            .await
        {
            Ok(_) => log::debug!("Autoreload server stopped"),
            Err(e) => log::error!("Autoreload {e}"),
        }
    }))
}

async fn websocket_handler(ws: WebSocketUpgrade, port: u16) -> impl IntoResponse {
    ws.on_upgrade(move |stream| websocket(stream, port))
}

async fn websocket(stream: WebSocket, port: u16) {
    let mut rx = MSG_BUS.subscribe();

    log::trace!("Autoreload websocket connected");
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(Msg::Reload(msg)) => {
                    if wait_for_localhost(port).await {
                        send_and_close(stream, &msg).await;
                    } else {
                        log::warn!(r#"Autoreload could not send "reload" to websocket"#);
                    }
                    break;
                }
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

async fn send_and_close(mut stream: WebSocket, msg: &str) {
    match stream.send(Message::Text(msg.to_string())).await {
        Err(e) => {
            log::debug!("Autoreload could not send {msg} due to {e}");
        }
        Ok(_) => {
            log::debug!("Autoreload sent \"{msg}\" to browser");
        }
    }
    if let Err(e) = stream.close().await {
        log::error!("Autoreload socket close error {e}");
    }
}
