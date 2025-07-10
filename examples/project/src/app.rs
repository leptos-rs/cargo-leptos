use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <button on:click=|_| spawn_local(on_click())>"click me"</button>
    }
}

#[lazy]
pub async fn on_click() {
    leptos::logging::log!("hello from a lazy function! hello from a lazy function! hello from a lazy function! hello from a lazy function! hello from a lazy function! hello from a lazy function! hello from a lazy function! hello from a lazy function! hello from a lazy function! hello from a lazy function! hello from a lazy function! hello from a lazy function! hello from a lazy function! hello from a lazy function! hello from a lazy function! hello from a lazy function!");
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
