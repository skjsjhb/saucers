use std::cell::RefCell;
use std::rc::Rc;

use saucers::app::App;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::webview::Webview;
use saucers::webview::events::NavigateEvent;

/// This example demonstrates how to open navigation requests that creates new windows.
/// By default, only in-page navigations are allowed. Saucer does not create a new window automatically when the
/// navigation requests one. To enable such behavior, the user will need to implement custom window managing.
fn main() {
    let (_cc, app) = App::new(AppOptions::new("Navigation"));

    let w = Webview::new(&Preferences::new(&app)).unwrap();

    // Window management happens only on the event thread, so `Rc` is sufficient.
    // The definition order here matters, as webview handles are stored in this list, it must be declared after the
    // collector, or the collector will complain when being dropped.
    let windows = Rc::new(RefCell::new(Vec::new()));

    w.set_size(1152, 648);
    w.set_url("https://saucer.app");

    w.on::<NavigateEvent>({
        let windows = windows.clone();
        let app = app.clone();

        Box::new(move |_, nav| {
            if nav.is_new_window() && nav.is_user_initiated() {
                // The event handler is fired on the event thread, so creating a window without posting is fine.
                let new_window = Webview::new(&Preferences::new(&app)).unwrap();
                new_window.set_url(nav.url());
                new_window.set_size(1152, 648);
                new_window.show();
                windows.borrow_mut().push(new_window);
            }
            true
        })
    });

    w.show();

    app.run();

    // Window list is cleared first, then the collector. Nice!
    // drop(windows);
    // drop(_cc);
}
