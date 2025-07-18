use saucers::app::App;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::webview::Webview;

/// This example demonstrates how to create a well-behaved empty app with minimum amount of code.
fn main() {
    let (_cc, app) = App::new(AppOptions::new("EmptyApp"));

    let w = Webview::new(&Preferences::new(&app)).unwrap();

    // Title bar may not display correctly if size is not set
    w.set_size(1152, 648);

    // Avoid using `about:blank` as it can cause some issues with internal scripts
    w.set_url("data:text/html,");
    w.show();

    app.run();
}
