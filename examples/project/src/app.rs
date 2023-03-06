use leptos::*;
use leptos_meta::*;

#[component]
pub fn App(cx: Scope) -> impl IntoView {
    provide_meta_context(cx);

    let welcome = format!("Hi from your Leptos WASM! ({})", message());
    view! {
        cx,
        <div>
            <Stylesheet id="leptos" href="/pkg/example.css" />
            <Title text="Cargo Leptos" />
            <h1>{welcome}</h1>
        </div>
    }
}

#[cfg(feature = "hydrate")]
use leptos::wasm_bindgen::prelude::wasm_bindgen;
#[cfg(feature = "hydrate")]
#[wasm_bindgen(module = "/js/foo.js")]
extern "C" {
    pub fn message() -> String;
}

#[cfg(not(feature = "hydrate"))]
#[allow(dead_code)]
pub fn message() -> String {
    "Rust".to_string()
}
