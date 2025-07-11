use saucers::app::App;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::webview::events::TitleEvent;
use saucers::webview::Webview;

/// This example shows a way to sync the window title with the page title.
/// This shows a way to use the webview event system.
fn main() {
    let (_cc, app) = App::new(AppOptions::new("AutoTitle"));

    let w = Webview::new(&Preferences::new(&app)).unwrap();

    w.set_size(1152, 648);

    w.on(
        TitleEvent,
        Box::new(|w, title| {
            w.set_title(title);
        })
    );

    w.set_url("https://saucer.app");
    w.show();

    app.run();
}
