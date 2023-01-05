use crate::app::*;
use actix_files::Files;
use actix_web::*;
use leptos::*;
use leptos_config::get_configuration;

pub async fn run() -> std::io::Result<()> {
    _ = dotenvy::dotenv();

    let conf = get_configuration(Some("Cargo.toml")).await.unwrap();
    let addr = conf.leptos_options.site_address;
    HttpServer::new(move || {
        let leptos_options = &conf.leptos_options;
        let site_root = &leptos_options.site_root;
        let pkg_dir = &leptos_options.site_pkg_dir;
        let bundle_path = format!("/{site_root}/{pkg_dir}");

        App::new()
            .service(Files::new(&bundle_path, format!("./{bundle_path}"))) // used by cargo-leptos. Can be removed if using wasm-pack and cargo run.
            .route("/api/{tail:.*}", leptos_actix::handle_server_fns())
            .route(
                "/{tail:.*}",
                leptos_actix::render_app_to_stream(
                    leptos_options.to_owned(),
                    |cx| view! { cx, <App/> },
                ),
            )
            .wrap(middleware::Compress::default())
    })
    .bind(&addr)?
    .run()
    .await
}
