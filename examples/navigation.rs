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

    w.set_size(1152, 648);
    w.set_url("https://saucer.app");

    w.on::<NavigateEvent>(Box::new(move |w, nav| {
        if nav.is_new_window() && nav.is_user_initiated() {
            // The event handler is fired on the event thread, so creating a window without posting is fine.
            let app = w.app();
            let new_window = Webview::new(&Preferences::new(&app)).unwrap();
            new_window.set_url(nav.url());
            new_window.set_size(1152, 648);
            new_window.show();
            // The app internally maintains a list of webviews, so dropping `new_window` won't destroy it.
        }
        true
    }));

    w.show();

    app.run();
}
