use saucers::app::App;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::webview::Webview;

/// This example demonstrates how to create a window with transparent/semi-transparent background.
/// Transparent windows work best with decorations disabled, but to keep it simple this example preserves control
/// widgets.
fn main() {
    let (_cc, app) = App::new(AppOptions::new("Transparent"));

    let w = Webview::new(&Preferences::new(&app)).unwrap();

    w.set_size(1152, 648);
    w.set_background(0, 0, 0, 0);

    w.set_url("about:blank");
    w.execute(
        r#"
        document.head.insertAdjacentHTML("beforeend", `
            <style>
                body {
                    background: #ffaec880;
                };
            </style>
        `);
    "#
    );

    w.show();

    app.run();
}
