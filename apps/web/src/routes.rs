//! Route table. **Add new pages here**: drop a component into `pages/`, add a
//! variant with a `#[route]` attribute, and link it from `app::App`.

use dioxus::prelude::*;

use crate::app::App;
use crate::pages::{About, Home};

#[derive(Debug, Clone, PartialEq, Routable)]
#[rustfmt::skip]
pub enum Route {
    #[layout(App)]
        #[route("/")]
        Home {},
        #[route("/about")]
        About {},
}
