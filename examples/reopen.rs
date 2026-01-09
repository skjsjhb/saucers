use std::cell::Cell;

use saucers::app::AppManager;
use saucers::app::AppOptions;
use saucers::policy::Policy;
use saucers::window::Window;
use saucers::window::WindowEventListener;
use saucers::NoOp;

/// This example shows how to listen for the close event and prevent the default behavior
/// conditionally.
fn main() {
    let app = AppManager::new(AppOptions::new_with_id("reopen"));

    app.run(
        |app, fin| {
            struct WindowEv {
                allow_close: Cell<bool>,
            }

            let window_ev = WindowEv { allow_close: Cell::new(false) };

            impl WindowEventListener for WindowEv {
                fn on_close(&self, _window: Window) -> Policy {
                    if self.allow_close.replace(true) {
                        println!("OK I'm closing.");
                        Policy::Allow
                    } else {
                        println!("Press again to close...");
                        Policy::Block
                    }
                }
            }

            let window = Window::new(&app, window_ev).unwrap();

            window.set_size((1152, 648));
            window.show();

            fin.set(|_| drop(window));
        },
        NoOp,
    )
    .unwrap();
}
