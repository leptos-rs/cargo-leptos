use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::{
    components::{FlatRoutes, Route, Router},
    lazy_route, Lazy, LazyRoute, StaticSegment,
};
use serde::{Deserialize, Serialize};

#[component]
pub fn App() -> impl IntoView {
    let count = RwSignal::new(0);
    provide_context(count);
    let (is_routing, set_is_routing) = signal(false);

    view! {
        <nav style="width: 100%">
            <a href="/">"A"</a> " | "
            <a href="/b">"B"</a> " | "
            <a href="/c">"C"</a>
            <span style="float: right">
                {move || is_routing.get().then_some("Loading...")}
            </span>
        </nav>
        <Router set_is_routing>
            <FlatRoutes fallback=|| "Not found.">
                <Route path=StaticSegment("") view=ViewA/>
                <Route path=StaticSegment("b") view=ViewB/>
                <Route path=StaticSegment("c") view={Lazy::<ViewC>::new()}/>
            </FlatRoutes>
        </Router>
    }
}

// View A: A plain old synchronous route, just like they all currently work. The WASM binary code
// for this is shipped as part of the main bundle.  Any data-loading code (like resources that run
// in the body of the component) will be shipped as part of the main bundle.

#[component]
pub fn ViewA() -> impl IntoView {
    leptos::logging::log!("View A");
    view! { <p>"View A"</p> }
}

// View B: lazy-loaded route with lazy-loaded data
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
pub fn ViewB() -> impl IntoView {
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
        <p>"View B"</p>
        <Suspense fallback=|| view! { <p>"Loading..."</p> }>
            <pre>{move || Suspend::new(async move {
                format!("{:#?}", data.await)
            })}</pre>
        </Suspense>
    }
    .into_any()
}

// View C: a lazy view, and some data, loaded in parallel when we navigate to /c.
#[derive(Clone)]
pub struct ViewC {
    //data: LocalResource<String>,
}

// Lazy-loaded routes need to implement the LazyRoute trait. They define a "route data" struct,
// which is created with `::data()`, and then a separate view function which is lazily loaded.
//
// This is important because it allows us to concurrently 1) load the route data, and 2) lazily
// load the component, rather than creating a "waterfall" where we can't start loading the route
// data until we've received the view.
//
// The `#[lazy_route]` macro makes `view` into a lazy-loaded inner function, replacing `self` with
// `this`.
#[lazy_route]
impl LazyRoute for ViewC {
    fn data() -> Self {
        Self {
            /* data: LocalResource::new(|| {
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
            }), */
        }
    }

    async fn view(self) -> AnyView {
        view! {
            <p>"View C"</p>
            <hr/>
            /* <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                <pre>{move || Suspend::new(async move {
                    format!("Loaded {} albums", this.data.await.len())
                })}</pre>
            </Suspense> */
        }
        .into_any()
    }
}
