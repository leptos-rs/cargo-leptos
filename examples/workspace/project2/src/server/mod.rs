use crate::app::*;
use actix_files::Files;
use actix_web::*;
use leptos::*;
use std::net;

fn app(cx: leptos::Scope) -> impl IntoView {
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
        let leptos_options = LeptosOptions::builder()
            .output_name("project2")
            .site_address(addr.clone())
            .site_root(&site_root)
            .site_pkg_dir(&pkg_dir)
            .build();

        App::new()
            .service(Files::new(&pkg_dir, format!("{site_root}/{pkg_dir}")))
            .wrap(middleware::Compress::default())
            .route(
                "/{tail:.*}",
                leptos_actix::render_app_to_stream(leptos_options, app),
            )
    })
    .bind(addr)?
    .run()
    .await
}
