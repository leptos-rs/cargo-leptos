mod app;
use cfg_if::cfg_if;

cfg_if! {
if #[cfg(feature = "hydrate")] {

  use wasm_bindgen::prelude::wasm_bindgen;

    #[wasm_bindgen]
    pub fn hydrate() {
      use app::*;
      use leptos::*;
      use leptos::logging::log;

      console_error_panic_hook::set_once();
      _ = console_log::init_with_level(log::Level::Debug);

      log!("hydrate mode - hydrating");

      leptos::mount_to_body(|| {
          view! {  <App/> }
      });
  }
}
}
