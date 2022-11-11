mod app;
#[cfg(feature = "ssr")]
mod server;

use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "csr")] {

        pub fn main() {
            use app::*;
            use leptos::*;

            _ = console_log::init_with_level(log::Level::Debug);
            mount_to_body(|cx| {
                view! { cx, <App /> }
            });
        }
    }
    else if #[cfg(feature = "ssr")] {

        #[actix_web::main]
        async fn main() -> std::io::Result<()> {
            server::run().await
        }
    }
}
