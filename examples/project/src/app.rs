use leptos::*;
use leptos_meta::*;

#[component]
pub fn App(cx: Scope) -> impl IntoView {
    provide_meta_context(cx);

    view! {
        cx,
        <Stylesheet id="leptos" href="/pkg/example.css" />
        <Title text="Cargo Leptos" />
        <main class="my-0 mx-auto max-w-3xl text-center">
            <h2 class="p-6 text-4xl">"Welcome to Leptos"</h2>
            <p class="px-10 pb-10 text-left">"This setup includes Tailwind and SASS"</p>
        </main>
    }
}

#[cfg(feature = "hydrate")]
use wasm_bindgen::prelude::wasm_bindgen;
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
