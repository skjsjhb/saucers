mod executor;
mod req;
mod res;

pub use executor::*;
pub use req::*;
pub use res::*;

use crate::capi::saucer_register_scheme;
use crate::macros::rtoc;

/// Registers a custom scheme.
///
/// This method must be called before creating any [`crate::app::App`] instances for the registration to take effect.
pub fn register_scheme(name: impl AsRef<str>) { rtoc!(name => n; saucer_register_scheme(n.as_ptr())) }
