use crate::{
    config::Project,
    ext::{sync::wait_for_socket, Paint},
    logger::GRAY,
    signal::{Interrupt, ReloadSignal, ReloadType},
};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use serde::Serialize;
use std::{fmt::Display, net::SocketAddr, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream},
    select,
    sync::RwLock,
    task::JoinHandle,
};

lazy_static::lazy_static! {
  static ref SITE_ADDR: RwLock<SocketAddr> = RwLock::new(SocketAddr::new([127,0,0,1].into(), 3000));
  static ref CSS_LINK: RwLock<String> = RwLock::new(String::default());
}

pub async fn spawn(proj: &Arc<Project>) -> JoinHandle<()> {
    let proj = proj.clone();

    let mut site_addr = SITE_ADDR.write().await;
    *site_addr = proj.site.addr;
    if let Some(file) = &proj.style.file {
        let mut css_link = CSS_LINK.write().await;
        // Always use `/` as separator in links
        *css_link = file
            .site
            .components()
            .map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join("/");
    }

    tokio::spawn(async move {
        let _change = ReloadSignal::subscribe();

        let reload_addr = proj.site.reload;

        if TcpStream::connect(&reload_addr).await.is_ok() {
            log::error!(
                    "Reload TCP port {reload_addr} already in use. You can set the port in the server integration's RenderOptions reload_port"
                );
            Interrupt::request_shutdown().await;

            return;
        }
        let route = Router::new().route("/live_reload", get(websocket_handler));

        log::debug!(
            "Reload server started {}",
            GRAY.paint(reload_addr.to_string())
        );

        match TcpListener::bind(&reload_addr).await {
            Ok(listener) => match axum::serve(listener, route).await {
                Ok(_) => log::debug!("Reload server stopped"),
                Err(e) => log::error!("Reload {e}"),
            },
            Err(e) => log::error!("Reload {e}"),
        }
    })
}

async fn websocket_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(websocket)
}

async fn websocket(mut stream: WebSocket) {
    let mut rx = ReloadSignal::subscribe();
    let mut int = Interrupt::subscribe_any();

    log::trace!("Reload websocket connected");
    tokio::spawn(async move {
        loop {
            select! {
                res = rx.recv() =>{
                    match res {
                        Ok(ReloadType::Full) => {
                            send_and_close(stream, BrowserMessage::all()).await;
                            return
                        }
                        Ok(ReloadType::Style) => {
                            send(&mut stream, BrowserMessage::css().await).await;
                        },
                        Ok(ReloadType::ViewPatches(data)) => {
                            send(&mut stream, BrowserMessage::view(data)).await;
                        }
                        Err(e) => log::debug!("Reload recive error {e}")
                    }
                }
                _ = int.recv(), if Interrupt::is_shutdown_requested().await => {
                    log::trace!("Reload websocket closed");
                    return
                },
            }
        }
    });
}

async fn send(stream: &mut WebSocket, msg: BrowserMessage) {
    let site_addr = *SITE_ADDR.read().await;
    if !wait_for_socket("Reload", site_addr).await {
        log::warn!(r#"Reload could not send "{msg}" to websocket"#);
    }

    let text = serde_json::to_string(&msg).unwrap();
    match stream.send(Message::Text(text.into())).await {
        Err(e) => {
            log::debug!("Reload could not send {msg} due to {e}");
        }
        Ok(_) => {
            log::debug!(r#"Reload sent "{msg}" to browser"#);
        }
    }
}

async fn send_and_close(mut stream: WebSocket, msg: BrowserMessage) {
    send(&mut stream, msg).await;
    let _ = stream.send(Message::Close(None)).await;
    log::trace!("Reload websocket closed");
}

#[derive(Serialize)]
struct BrowserMessage {
    css: Option<String>,
    view: Option<String>,
    all: bool,
}

impl BrowserMessage {
    async fn css() -> Self {
        let link = CSS_LINK.read().await.clone();
        if link.is_empty() {
            log::error!("Reload internal error: sending css reload but no css file is set.");
        }
        Self {
            css: Some(link),
            view: None,
            all: false,
        }
    }

    fn view(data: String) -> Self {
        Self {
            css: None,
            view: Some(data),
            all: false,
        }
    }

    fn all() -> Self {
        Self {
            css: None,
            view: None,
            all: true,
        }
    }
}

impl Display for BrowserMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(css) = &self.css {
            write!(f, "reload {}", css)
        } else {
            write!(f, "reload all")
        }
    }
}
