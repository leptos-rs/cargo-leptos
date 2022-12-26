#[cfg(feature = "ssr")]
mod actix_integ;

#[cfg(feature = "ssr")]
use actix_web::*;
#[cfg(feature = "ssr")]
use leptos::*;

#[cfg(feature = "ssr")]
fn app(cx: leptos::Scope) -> Element {
    use app_package::*;

    view! { cx, <App /> }
}

#[cfg(feature = "ssr")]
#[actix_web::main]
pub async fn main() -> std::io::Result<()> {
    use actix_files::Files;
    use std::net;

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

#[cfg(not(feature = "ssr"))]
fn main() {}
