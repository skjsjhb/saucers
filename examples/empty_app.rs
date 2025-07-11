use saucers::app::App;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::webview::Webview;

fn main() {
    let (_cc, app) = App::new(AppOptions::new("EmptyApp"));

    let w = Webview::new(&Preferences::new(&app)).unwrap();

    // Title bar may not display correctly if size is not set
    w.set_size(1152, 648);

    w.set_url("about:blank");
    w.show();

    app.run();
}
