use leptos::*;
use leptos_meta::*;

#[component]
pub fn App(cx: Scope) -> impl IntoView {
    provide_meta_context(cx);

    view! {
        cx,
        <div>
            <Title text="Cargo Leptos" />
            <h1>"Hi from your Leptos WASM!"</h1>
        </div>
    }
}
