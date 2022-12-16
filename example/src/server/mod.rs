mod actix_integ;

use crate::app::*;
use actix_files::Files;
use actix_web::*;
use leptos::*;
use leptos_router::*;
use std::net;

#[derive(Copy, Clone, Debug)]
struct ActixIntegration {
    path: ReadSignal<String>,
}

impl History for ActixIntegration {
    fn location(&self, cx: leptos::Scope) -> ReadSignal<LocationChange> {
        create_signal(
            cx,
            LocationChange {
                value: self.path.get(),
                replace: false,
                scroll: true,
                state: State(None),
            },
        )
        .0
    }

    fn navigate(&self, _loc: &LocationChange) {}
}

fn app(cx: leptos::Scope) -> Element {
    view! { cx, <App /> }
}

pub async fn run() -> std::io::Result<()> {
    _ = dotenvy::dotenv();

    let addr: net::SocketAddr = std::env::var("LEPTOS_SITE_ADDR").unwrap().parse().unwrap();

    simple_logger::init_with_level(log::Level::Debug).expect("couldn't initialize logging");

    log::info!("serving at {addr}");

    let site_root = std::env::var("LEPTOS_SITE_ROOT").unwrap();
    let pkg_dir = std::env::var("LEPTOS_SITE_PKG_DIR").unwrap();

    HttpServer::new(move || {
        App::new()
            .service(Files::new(&pkg_dir, format!("{site_root}/{pkg_dir}")))
            .wrap(middleware::Compress::default())
            .route("/{tail:.*}", actix_integ::render_app_to_stream(app))
    })
    .bind(&addr)?
    .run()
    .await
}
