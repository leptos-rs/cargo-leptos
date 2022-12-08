use std::net::SocketAddr;

use crate::ext::sync::{runconfig_changed_or_shutdown, wait_for, SHUTDOWN};
use crate::ext::util::SenderAdditions;
use crate::logger::GRAY;
use crate::run::run_config;
use crate::sync::{oneshot_when, wait_for_localhost};
use crate::{Msg, MSG_BUS};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use tokio::{net::TcpStream, task::JoinHandle};

pub async fn spawn() -> JoinHandle<()> {
    tokio::spawn(async {
        wait_for(runconfig_changed_or_shutdown).await;

        while !*SHUTDOWN.read().await {
            let shutdown_rx = oneshot_when(runconfig_changed_or_shutdown, "LiveReload");

            let port = run_config::RUN_CONFIG.read().await.reload_port;
            let addr = SocketAddr::from(([127, 0, 0, 1], port));

            if let Ok(_) = TcpStream::connect(&addr).await {
                log::error!(
                    "LiveReload TCP port {addr} already in use. You can set the port in the server integration's RenderOptions reload_port"
                );
                MSG_BUS.send_logged("LiveReload", Msg::ShutDown);
                return;
            }
            let route = Router::new().route("/live_reload", get(move |ws| websocket_handler(ws)));

            log::debug!("LiveReload server started {}", GRAY.paint(addr.to_string()));

            match axum::Server::bind(&addr)
                .serve(route.into_make_service())
                .with_graceful_shutdown(async move { drop(shutdown_rx.await.ok()) })
                .await
            {
                Ok(_) => log::debug!("LiveReload server stopped"),
                Err(e) => log::error!("LiveReload {e}"),
            }
        }
    })
}

async fn websocket_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |stream| websocket(stream))
}

async fn websocket(stream: WebSocket) {
    let mut rx = MSG_BUS.subscribe();

    log::trace!("LiveReload websocket connected");
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(Msg::Reload(msg)) => {
                    let port = run_config::RUN_CONFIG.read().await.server_addr.port();
                    if wait_for_localhost(port).await {
                        send_and_close(stream, &msg).await;
                    } else {
                        log::warn!(r#"LiveReload could not send "reload" to websocket"#);
                    }
                    break;
                }
                Err(e) => {
                    log::debug!("LiveReload recive error {e}");
                    break;
                }
                Ok(Msg::ShutDown) => break,
                _ => {}
            }
        }
        log::trace!("LiveReload websocket closed")
    });
}

async fn send_and_close(mut stream: WebSocket, msg: &str) {
    match stream.send(Message::Text(msg.to_string())).await {
        Err(e) => {
            log::debug!("LiveReload could not send {msg} due to {e}");
        }
        Ok(_) => {
            log::debug!("LiveReload sent \"{msg}\" to browser");
        }
    }
    if let Err(e) = stream.close().await {
        log::error!("LiveReload socket close error {e}");
    }
}
