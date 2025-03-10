use crate::app::*;
use actix_files::Files;
use actix_web::{App as ActixApp, *};
use leptos::{config::get_configuration, logging::warn, prelude::*};
use leptos_actix::{generate_route_list, LeptosRoutes};
use leptos_meta::MetaTags;

pub async fn run() -> std::io::Result<()> {
    _ = dotenvy::dotenv();

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;

    warn!("serving at {addr}");

    HttpServer::new(move || {
        // Generate the list of routes in your Leptos App
        let routes = generate_route_list(App);
        let leptos_options = &conf.leptos_options;
        let site_root = &leptos_options.site_root;

        ActixApp::new()
            .leptos_routes(routes, {
                let options = leptos_options.clone();

                move || {
                    view! {
                        <!DOCTYPE html>
                        <html lang="en">
                            <head>
                                <meta charset="utf-8" />
                                <meta
                                    name="viewport"
                                    content="width=device-width, initial-scale=1"
                                />
                                <AutoReload options=options.clone() />
                                <HydrationScripts options=options.clone() />
                                <MetaTags />
                            </head>
                            <body>
                                <App />
                            </body>
                        </html>
                    }
                }
            })
            .service(Files::new("/", site_root.as_ref()))
            .wrap(middleware::Compress::default())
    })
    .bind(&addr)?
    .run()
    .await
}
