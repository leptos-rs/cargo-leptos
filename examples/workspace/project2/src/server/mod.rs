use crate::app::*;
use actix_files::Files;
use actix_web::*;
use leptos::*;
use leptos_actix::{generate_route_list, LeptosRoutes};

fn app(cx: leptos::Scope) -> impl IntoView {
    view! { cx, <App /> }
}

pub async fn run() -> std::io::Result<()> {
    _ = dotenvy::dotenv();

    simple_logger::init_with_level(log::Level::Debug).expect("couldn't initialize logging");

    let conf = get_configuration(None).await.unwrap();
    let addr = conf.leptos_options.site_addr.clone();

    log::info!("serving at {addr}");

    // Generate the list of routes in your Leptos App
    let routes = generate_route_list(app);

    HttpServer::new(move || {
        let leptos_options = &conf.leptos_options;

        let pkg_dir = leptos_options.site_pkg_dir.clone();
        let site_root = leptos_options.site_root.clone();
        App::new()
            .leptos_routes(leptos_options.to_owned(), routes.to_owned(), app)
            .service(Files::new(&pkg_dir, format!("{site_root}/{pkg_dir}")))
            .wrap(middleware::Compress::default())
    })
    .bind(addr)?
    .run()
    .await
}
