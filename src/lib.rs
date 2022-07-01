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
pub mod sessions;
pub mod tests;
pub mod tracing;
pub mod validation;

#[cfg(feature = "img_processing")]
pub mod image_compression;
#[cfg(feature = "zip")]
pub mod zip;

#[macro_use]
pub mod macro_helpers;
pub use macro_helpers::*;

pub use paste;

pub mod reexports {
    pub use axum_extra::routing::TypedPath;
    pub use serde::Deserialize;
}
