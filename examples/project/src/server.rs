use crate::app::*;
use actix_files::Files;
use actix_web::*;
use leptos::*;
use leptos_actix::{generate_route_list, LeptosRoutes};

pub async fn run() -> std::io::Result<()> {
    _ = dotenvy::dotenv();

    let conf = get_configuration(None).await.unwrap();
    let addr = conf.leptos_options.site_addr.clone();

    log::info!("serving at {addr}");

    // Generate the list of routes in your Leptos App
    let routes = generate_route_list(|cx| view! { cx, <App/> });

    HttpServer::new(move || {
        let leptos_options = &conf.leptos_options;

        let site_root = leptos_options.site_root.clone();

        App::new()
            .leptos_routes(
                leptos_options.to_owned(),
                routes.to_owned(),
                |cx| view! { cx, <App/> },
            )
            .service(Files::new("/", site_root.to_owned()))
            .wrap(middleware::Compress::default())
    })
    .bind(&addr)?
    .run()
    .await
}
