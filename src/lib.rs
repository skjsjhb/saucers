//! This is the Rust bindings for [saucer](https://github.com/saucer/saucer). The C++ webview
//! library.
//!
//! This crate wraps around the C API of saucer and intends to provide safe items for using directly
//! or as building blocks of frameworks.
//!
//! Examples can be found in the [`examples`](https://github.com/skjsjhb/saucers/tree/main/examples)
//! directory.

use std::ffi::CStr;

use saucer_sys::saucer_version;

use crate::app::AppEventListener;
use crate::webview::WebviewEventListener;
use crate::webview::WebviewSchemeHandler;
use crate::window::WindowEventListener;

pub mod app;
pub mod desktop;
pub mod error;
pub mod icon;
mod macros;
pub mod navigation;
pub mod pdf;
pub mod permission;
pub mod policy;
pub mod scheme;
pub mod screen;
pub mod stash;
pub mod state;
pub mod status;
pub mod url;
mod util;
pub mod webview;
pub mod window;

/// Gets the library version. Returns an empty string if the version can't be determined.
pub fn version() -> &'static str {
    unsafe { CStr::from_ptr(saucer_version()).to_str().unwrap_or("") }
}

/// A ZST for ignoring all events.
pub struct NoOp;

impl AppEventListener for NoOp {}
impl WindowEventListener for NoOp {}
impl WebviewEventListener for NoOp {}
impl WebviewSchemeHandler for NoOp {}
