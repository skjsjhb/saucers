use saucers::app::App;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::webview::Webview;

#[test]
fn test_webview() {
    let app = App::new(AppOptions::new("saucer"));
    let wv = Webview::new(&Preferences::new(&app)).unwrap();
    wv.set_url("https://github.com");
    wv.show();
    app.require_main().unwrap().run();
}
