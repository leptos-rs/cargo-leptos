use crate::app::*;
use actix_files::Files;
use actix_web::*;
use leptos::prelude::*;
use leptos_actix::{generate_route_list, LeptosRoutes};
use log::info;

fn app() -> impl IntoView {
    view! { <App /> }
}

pub async fn run() -> std::io::Result<()> {
    _ = dotenvy::dotenv();

    simple_logger::init_with_level(log::Level::Debug).expect("couldn't initialize logging");

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;

    info!("serving at {addr}");

    // Generate the list of routes in your Leptos App
    let routes = generate_route_list(app);

    HttpServer::new(move || {
        let leptos_options = &conf.leptos_options;

        let pkg_dir = leptos_options.site_pkg_dir.clone();
        let site_root = leptos_options.site_root.clone();
        App::new()
            .leptos_routes(
              routes.clone(), {
                let leptos_options = leptos_options.clone();
                move || view!{
                  <App/>
                }
              }
            )
            .service(Files::new(&pkg_dir, format!("{site_root}/{pkg_dir}")))
            .wrap(middleware::Compress::default())
    })
    .bind(addr)?
    .run()
    .await
}
