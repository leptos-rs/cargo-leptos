use actix_files::{Files, NamedFile};
use actix_web::*;
use example_app::*;
use futures::StreamExt;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;

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

#[get("/static/style.css")]
async fn css() -> impl Responder {
    NamedFile::open_async("../lib/style.css").await
}

// match every path — our router will handle actual dispatch
#[get("{tail:.*}")]
async fn render_app(req: HttpRequest) -> impl Responder {
    let path = req.path();

    let query = req.query_string();
    let path = if query.is_empty() {
        "http://leptos".to_string() + path
    } else {
        "http://leptos".to_string() + path + "?" + query
    };

    let app = move |cx| {
        let integration = ActixIntegration {
            path: create_signal(cx, path.clone()).0,
        };
        provide_context(cx, RouterIntegrationContext(std::rc::Rc::new(integration)));

        view! { cx, <App /> }
    };

    let head = r#"<!DOCTYPE html>
            <html lang="en">
                <head>
                    <meta charset="utf-8"/>
                    <meta name="viewport" content="width=device-width, initial-scale=1"/>
                    <script type="module">import init, { main } from '/pkg/polyglot_client.js'; init().then(main);</script>"#;
    let tail = "</body></html>";

    HttpResponse::Ok().content_type("text/html").streaming(
        futures::stream::once(async { head.to_string() })
            .chain(render_to_stream(move |cx| {
                let app = app(cx);
                let head = use_context::<MetaContext>(cx)
                    .map(|meta| meta.dehydrate())
                    .unwrap_or_default();
                format!("{head}</head><body>{app}")
            }))
            .chain(futures::stream::once(async { tail.to_string() }))
            .map(|html| Ok(web::Bytes::from(html)) as Result<web::Bytes>),
    )
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .unwrap();
    log::debug!("serving at {host}:{port}");

    simple_logger::init_with_level(log::Level::Debug).expect("couldn't initialize logging");

    HttpServer::new(|| {
        App::new()
            .service(css)
            .service(
                web::scope("/pkg")
                    .service(Files::new("", "../client/pkg"))
                    .wrap(middleware::Compress::default()),
            )
            .service(render_app)
    })
    .bind((host, port))?
    .run()
    .await
}
