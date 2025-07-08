mod executor;
mod req;
mod res;

pub use executor::*;
pub use req::*;
pub use res::*;

use crate::capi::saucer_register_scheme;
use crate::rtoc;

pub fn register_scheme(name: impl AsRef<str>) { rtoc!(name => n; saucer_register_scheme(n.as_ptr())) }
