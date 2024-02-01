FROM --platform=amd64 rust:1.71 as build

RUN cargo install cargo-leptos

FROM --platform=amd64 rust:1.71

COPY --from=build /usr/local/cargo/bin/cargo-leptos /usr/local/cargo/bin/cargo-leptos

ENTRYPOINT ["cargo-leptos"]
