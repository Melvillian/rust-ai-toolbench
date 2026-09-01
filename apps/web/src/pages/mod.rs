//! One component per route. Register new pages in `crate::routes`.

mod about;
mod home;

pub use about::About;
pub use home::Home;
