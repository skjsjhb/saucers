use saucers::app::App;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::webview::Webview;
use saucers::webview::events::FaviconEvent;
use saucers::webview::events::TitleEvent;

/// This example shows a way to sync the window title and icon with the web page.
/// This shows a way to use the webview event system.
fn main() {
    let (_cc, app) = App::new(AppOptions::new("SyncTitleIcon"));

    let w = Webview::new(&Preferences::new(&app)).unwrap();

    w.set_size(1152, 648);

    w.on::<TitleEvent>(Box::new(|w, title| {
        w.set_title(title);
    }));

    w.on::<FaviconEvent>(Box::new(|w, icon| {
        w.set_icon(icon);
    }));

    w.set_url("https://saucer.app");
    w.show();

    app.run();
}
