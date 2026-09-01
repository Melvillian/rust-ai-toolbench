//! Entrypoint: mounts the router. Pages live in `pages/`, the route table in
//! `routes.rs`, and the layout shell in `app.rs`.

mod app;
mod pages;
mod routes;

use dioxus::prelude::*;

use routes::Route;

const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

fn main() {
    dioxus::launch(Root);
}

/// Top-level component: global stylesheet + the router.
#[component]
fn Root() -> Element {
    rsx! {
        document::Stylesheet { href: TAILWIND_CSS }
        Router::<Route> {}
    }
}
