mod actix_integ;

use crate::app::*;
use actix_files::Files;
use actix_web::*;
use leptos::*;
use leptos_router::*;
use std::{env, net};

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
    let addr = net::SocketAddr::from(([127, 0, 0, 1], 3000));

    simple_logger::init_with_level(log::Level::Debug).expect("couldn't initialize logging");

    log::info!("serving at {addr}");

    HttpServer::new(move || {
        let render_options: RenderOptions = RenderOptions::builder()
            .pkg_path("/pkg/app")
            .reload_port(3001)
            .socket_address(addr.clone())
            .environment(&env::var("RUST_ENV"))
            .build();
        render_options.write_to_file();

        App::new()
            .service(Files::new("/pkg", "target/site/pkg"))
            .wrap(middleware::Compress::default())
            .route(
                "/{tail:.*}",
                actix_integ::render_app_to_stream(render_options, app),
            )
    })
    .bind(&addr)?
    .run()
    .await
}
