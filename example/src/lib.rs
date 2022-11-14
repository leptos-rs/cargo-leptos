mod app;
use cfg_if::cfg_if;

// Needs to be in lib.rs AFAIK because wasm-bindgen needs us to be compiling a lib. I may be wrong.
cfg_if! {
if #[cfg(feature = "hydrate")] {

  use wasm_bindgen::prelude::wasm_bindgen;

    #[wasm_bindgen(start)]
    pub fn main() {
      use app::*;
      use leptos::*;

      _ = console_log::init_with_level(log::Level::Debug);
      console_error_panic_hook::set_once();

      log!("hydrate mode - hydrating");

      leptos::hydrate(body().unwrap(), move |cx| {
        view! { cx, <App/> }
      });
    }
}
  else if #[cfg(feature = "csr")] {

    use wasm_bindgen::prelude::wasm_bindgen;

    #[wasm_bindgen(start)]
    pub fn main() {
        use app::*;
        use leptos::*;
        _ = console_log::init_with_level(log::Level::Debug);
        console_error_panic_hook::set_once();

        log!("csr mode - mounting to body");

        mount_to_body(|cx| {
            view! { cx, <App /> }
        });
    }
  }
}
