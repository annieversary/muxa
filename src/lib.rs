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
pub mod zip;

#[cfg(feature = "img_processing")]
pub mod image_compression;

#[macro_use]
pub mod macro_helpers;
pub use macro_helpers::*;

pub use paste;
