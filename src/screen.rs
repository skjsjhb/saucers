use saucer_sys::saucer_screen;
use saucer_sys::saucer_screen_free;
use saucer_sys::saucer_screen_name;
use saucer_sys::saucer_screen_position;
use saucer_sys::saucer_screen_size;

use crate::util::make_owned_string;

pub struct Screen {
    pub name: String,
    pub size: (i32, i32),
    pub pos: (i32, i32),
}

impl Screen {
    /// Takes the given raw pointer and converts it into a [`Screen`]. Returns [`None`] if the
    /// pointer is null.
    pub(crate) unsafe fn from_raw(ptr: *mut saucer_screen) -> Option<Self> {
        if ptr.is_null() {
            return None;
        }

        let mut w = 0;
        let mut h = 0;
        let mut x = 0;
        let mut y = 0;

        unsafe {
            saucer_screen_size(ptr, &raw mut w, &raw mut h);
            saucer_screen_position(ptr, &raw mut x, &raw mut y);
        }

        let name = unsafe { make_owned_string(saucer_screen_name(ptr)) }; // The name is borrowed

        unsafe { saucer_screen_free(ptr) };

        Some(Self { name, size: (w, h), pos: (x, y) })
    }
}
