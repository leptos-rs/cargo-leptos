use wasm_bindgen::prelude::wasm_bindgen;

use app_package::*;
use leptos::prelude::*;

#[wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    _ = console_log::init_with_level(log::Level::Debug);

    leptos::logging::log!("hydrate mode - hydrating");

    mount_to_body(|| {
        view! { <App/> }
    });
}
