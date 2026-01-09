//! Scheme handling module.
//!
//! This module includes [`Executor`], [`Request`] and [`Response`] to handle requests to custom
//! schemes.
mod executor;
mod request;
mod response;

pub use executor::*;
pub use request::*;
pub use response::*;
use saucer_sys::saucer_webview_register_scheme;

use crate::macros::use_string;

/// Registers a custom scheme.
///
/// This method must be called before creating any app handles to take effect.
pub fn register_scheme(name: impl Into<Vec<u8>>) {
    use_string!(n: name; unsafe { saucer_webview_register_scheme(n) });
}
