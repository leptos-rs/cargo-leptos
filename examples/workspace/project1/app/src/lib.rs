use leptos::*;
use leptos_meta::*;

#[component]
pub fn App() -> impl IntoView {

    view! {
        <div>
            <Stylesheet id="leptos" href="/pkg/project1.css"/>
            <Title text="Cargo Leptos" />
            <h1>"Hi from your Leptos WASM!"</h1>
        </div>
    }
}
