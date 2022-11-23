use crate::logger::GRAY;
use crate::sync::{oneshot_when, shutdown_msg};
use crate::{config::Config, Msg, MSG_BUS};
use ansi_term::Style;
use anyhow_ext::{bail, Result};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use std::{net::SocketAddr, time::Duration};
use tokio::{net::TcpStream, task::JoinHandle, time::sleep};

pub async fn spawn(config: &Config) -> Result<JoinHandle<()>> {
    let port = config.leptos.csr_port;
    let route = Router::new().route("/autoreload", get(move |ws| websocket_handler(ws, port)));

    let addr = SocketAddr::from(([127, 0, 0, 1], config.leptos.reload_port));

    if let Ok(_) = TcpStream::connect(&addr).await {
        let bold = Style::new().bold();
        bail!(
            "Server port {} already in use. You can set which port to use with {} in {} section {}",
            config.leptos.reload_port,
            bold.paint("reload_port"),
            bold.paint("Cargo.toml"),
            bold.paint("[package.metadata.leptos]"),
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
                    if wait_for_port(port).await {
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

async fn wait_for_port(port: u16) -> bool {
    let duration = Duration::from_millis(500);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    for _ in 0..20 {
        if let Ok(_) = TcpStream::connect(&addr).await {
            log::trace!("Autoreload server port {port} open");
            return true;
        }
        sleep(duration).await;
    }
    log::warn!("Autoreload timed out waiting for port {port}");
    false
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
