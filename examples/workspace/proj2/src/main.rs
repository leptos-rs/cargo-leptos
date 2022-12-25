mod app;
#[cfg(and(feature = "ssr", not(feature = "hydrate")))]
mod server;

use cfg_if::cfg_if;

cfg_if! {
    #[cfg(and(feature = "ssr", not(feature = "hydrate")))] {
        #[actix_web::main]
        async fn main() -> std::io::Result<()> {
            server::run().await
        }
    }
    else {
        pub fn main() {}
    }
}
