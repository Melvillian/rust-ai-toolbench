//! Layout shell: navigation + an `Outlet` for the routed page. Kept free of
//! page content so `routes.rs` stays the single place pages are registered.

use dioxus::prelude::*;

use crate::routes::Route;

#[component]
pub fn App() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-50 text-gray-900",
            nav { class: "flex gap-6 border-b border-gray-200 bg-white px-6 py-4",
                Link { to: Route::Home {}, class: "font-semibold hover:text-blue-600", "Home" }
                Link { to: Route::About {}, class: "font-semibold hover:text-blue-600", "About" }
            }
            main { class: "mx-auto max-w-2xl p-6",
                Outlet::<Route> {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use dioxus::history::MemoryHistory;
    use dioxus::prelude::*;
    use dioxus::router::components::HistoryProvider;

    use crate::routes::Route;

    /// Render the whole app (router + layout + page) to HTML for `path`.
    fn render_at(path: &str) -> String {
        let mut dom = VirtualDom::new_with_props(
            Shell,
            ShellProps {
                path: path.to_owned(),
            },
        );
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    /// Native test builds have no browser history, so provide an in-memory
    /// one that starts at `path`.
    #[component]
    fn Shell(path: String) -> Element {
        rsx! {
            HistoryProvider {
                history: move |()| Rc::new(MemoryHistory::with_initial_path(&path)) as Rc<dyn History>,
                Router::<Route> {}
            }
        }
    }

    #[test]
    fn renders_the_nav_and_the_routed_page() {
        let html = render_at("/about");
        assert!(html.contains(r#"href="/""#), "{html}");
        assert!(html.contains(r#"href="/about""#), "{html}");
        assert!(html.contains(">Home<"), "{html}");
        assert!(html.contains(">About<"), "{html}");
        assert!(html.contains("About this template"), "{html}");
    }

    #[test]
    fn home_route_renders_the_home_page() {
        let html = render_at("/");
        assert!(html.contains("<h1"), "{html}");
        assert!(html.contains("/api/hello"), "{html}");
    }
}
