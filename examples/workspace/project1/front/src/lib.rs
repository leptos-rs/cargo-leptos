use wasm_bindgen::prelude::wasm_bindgen;

use app_package::*;
use leptos::*;

#[wasm_bindgen]
pub fn hydrate() {
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();

    log::info!("hydrate mode - hydrating");

    leptos::hydrate(body().unwrap(), move |cx| {
        view! { cx, <App/> }
    });
}
