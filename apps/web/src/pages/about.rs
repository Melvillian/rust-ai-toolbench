use dioxus::prelude::*;

#[component]
pub fn About() -> Element {
    rsx! {
        section {
            h1 { class: "mb-4 text-2xl font-bold", "About this template" }
            p {
                "This is the second route of the starter app, here to show that "
                "client-side routing works end to end (including the SPA fallback on "
                "page refresh). Add a page by dropping a component into "
                code { "src/pages/" }
                " and registering it in "
                code { "src/routes.rs" }
                "."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use dioxus::prelude::*;

    use super::About;

    #[test]
    fn renders_heading_and_instructions() {
        let html = dioxus_ssr::render_element(rsx! { About {} });
        assert!(html.contains("About this template"), "{html}");
        assert!(html.contains("src/routes.rs"), "{html}");
    }
}
