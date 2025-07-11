use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::{
    components::{FlatRoutes, Route, Router},
    lazy_route, Lazy, LazyRoute, StaticSegment,
};
use serde::{Deserialize, Serialize};

// The root component of the application.
// This component sets up the router and navigation links.
#[component]
pub fn App() -> impl IntoView {
    let count = RwSignal::new(0);
    provide_context(count);
    let (is_routing, set_is_routing) = signal(false);

    view! {
        <nav style="width: 100%">
            <a href="/">"Home"</a> " | "
            <a href="/lazy-data">"Lazy Data"</a> " | "
            <a href="/lazy-view-and-data">"Lazy View and Data"</a>

            <span style="float: right">
                {move || is_routing.get().then_some("Loading...")}
            </span>
        </nav>
        <Router set_is_routing>
            <FlatRoutes fallback=|| "Not found.">
                <Route path=StaticSegment("") view=HomePage />
                <Route path=StaticSegment("lazy-data") view=LazyDataPage />
                <Route path=StaticSegment("lazy-view-and-data") view={Lazy::<LazyViewAndDataPage>::new()}/>
            </FlatRoutes>
        </Router>
    }
}


// The home page of the application.
// It demonstrates how to use `#[lazy]` on an async function `lazy_value`
// to split the code into a separate WASM module.
#[component]
pub fn HomePage() -> impl IntoView {
    let data = RwSignal::new(String::new());

    view! {
        <h1>"SSR App with Code Splitting"</h1>

        <button on:click=move |_| spawn_local(async move {
            *data.write() = lazy_value().await;
        })>"Load Lazy Function"</button>

        <p>Lazy Loaded Function Data with Serialize: {data}</p>
    }
}

#[lazy]
async fn lazy_value() -> String {
    use serde::Serialize;

    #[derive(Serialize)]
    struct SomeData {
        foo: String,
        bar: i32,
        baz: bool,
    }

    serde_json::to_string(&SomeData {
        foo: "This is a test".into(),
        bar: 42,
        baz: true,
    })
    .unwrap_or_else(|e| e.to_string())
}

// Demonstrates a route that fetches data lazily.
// The `deserialize_comments` function is marked with `#[lazy]`,
// so its code is loaded only when needed.
#[derive(Debug, Clone, Deserialize)]
pub struct Comment {
    #[serde(rename = "postId")]
    post_id: usize,
    id: usize,
    name: String,
    email: String,
    body: String,
}

#[lazy]
async fn deserialize_comments(data: &str) -> Vec<Comment> {
    serde_json::from_str(data).unwrap()
}

#[component]
pub fn LazyDataPage() -> impl IntoView {
    let data = LocalResource::new(|| async move {
        let preload = deserialize_comments("[]");
        let (_, data) = futures::future::join(preload, async {
            gloo_net::http::Request::get("https://jsonplaceholder.typicode.com/comments")
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        })
        .await;
        deserialize_comments(&data).await
    });

    view! {
        <p>"Lazy Data Page"</p>
        <Suspense fallback=|| view! { <p>"Loading..."</p> }>
            <pre>{move || Suspend::new(async move {
                format!("{:#?}", data.await)
            })}</pre>
        </Suspense>
    }
    .into_any()
}


// Lazy-loaded routes need to implement the LazyRoute trait. They define a "route data" struct,
// which is created with `::data()`, and then a separate view function which is lazily loaded.
//
// This is important because it allows us to concurrently 1) load the route data, and 2) lazily
// load the component, rather than creating a "waterfall" where we can't start loading the route
// data until we've received the view.
#[derive(Clone)]
pub struct LazyViewAndDataPage {
    data: LocalResource<String>,
}

// The `#[lazy_route]` macro makes `view` into a lazy-loaded inner function, replacing `self` with `this`.
#[lazy_route]
impl LazyRoute for LazyViewAndDataPage {
    fn data() -> Self {
        Self {
            data: LocalResource::new(|| {
                leptos::logging::log!("calling out to API");
                async {
                    gloo_net::http::Request::get("https://jsonplaceholder.typicode.com/albums")
                        .send()
                        .await
                        .unwrap()
                        .text()
                        .await
                        .unwrap()
                }
            }),
        }
    }

    async fn view(self) -> AnyView {
        view! {
            <p>"Lazy View and Data Page"</p>
            <hr/>
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                <pre>{move || Suspend::new(async move {
                    format!("Loaded {} albums", this.data.await.len())
                })}</pre>
            </Suspense>
        }
        .into_any()
    }
}
