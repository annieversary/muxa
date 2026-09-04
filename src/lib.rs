#![allow(clippy::too_many_arguments)]

#[macro_use]
extern crate serde;

pub mod config;
pub mod consts;
pub mod cookies;
pub mod errors;
pub mod extractors;
pub mod helpers;
pub mod html;
pub mod router;
pub mod sessions;
pub mod tests;
pub mod theme;
pub mod tracing;
pub mod validation;

#[cfg(feature = "zephyr")]
pub mod css;
#[cfg(feature = "img_processing")]
pub mod image_compression;
#[cfg(feature = "zip")]
pub mod zip;

// the macros in here are `#[macro_export]`ed, so they already live at the crate root
pub mod macro_helpers;

pub use paste;

/// everything the `routes!` and `default_layers!` macros expand to, so that a crate
/// using them only needs to depend on muxa
pub mod reexports {
    pub use axum_extra::routing::TypedPath;
    pub use serde::Deserialize;

    pub use const_random::const_random;

    pub use axum;
    pub use http;
    pub use maud;
    pub use tower;
    pub use tower_http;
}
