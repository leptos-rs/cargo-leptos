use leptos::*;
use leptos_meta::*;

#[component]
pub fn App(cx: Scope) -> Element {
    provide_context(cx, MetaContext::default());

    view! {
        cx,
        <div>
            <Stylesheet href="/static/style.css".into()/>
            <div>"Hello world"</div>
        </div>
    }
}
