# SSR Code Splitting Example

This example demonstrates how to use code splitting in a Leptos application with Server-Side Rendering (SSR).

Code splitting is a technique used to split your application's code into smaller chunks, which can be loaded on demand. This can significantly improve the initial load time of your application, as the user only needs to download the code necessary for the initial view.

## Features Demonstrated

- **`#[lazy]` on functions**: The `HomePage` shows how to use the `#[lazy]` macro on an `async fn` to defer loading of that function's code until it's called.
- **`#[lazy_route]` on components**: The `LazyViewAndDataPage` demonstrates how to use the `LazyRoute` trait to create routes where both the view component and its associated data are loaded lazily and concurrently when the user navigates to that route.

## How to Run

To run this example, use `cargo-leptos` binary:

```bash
cargo leptos watch --split
```

Or to run with `cargo-leptos` sources:

```bash
cargo run -- --manifest-path examples/ssr-splitting/Cargo.toml watch --split
```

Then, open your browser to `http://127.0.0.1:3000` and observe the network requests in your browser's developer tools as you navigate between pages.
