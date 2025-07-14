use saucers::app::App;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::webview::Webview;

/// This example demonstrates how to create a frameless window and use the `data-webview-drag` attribute to allow
/// dragging the window using an HTML element.
fn main() {
    let (_cc, app) = App::new(AppOptions::new("Frameless"));

    let w = Webview::new(&Preferences::new(&app)).unwrap();

    w.set_decorations(false);
    w.set_size(1152, 648);

    w.set_url("data:text/html,");

    // Add a button which can be used to drag the window.
    // There currently isn't an attribute for using an element as the close button. Consider using messages for this.
    w.execute(
        r#"
        document.body.innerHTML = `
            <button data-webview-drag>Drag Window</button>
        `;
    "#
    );

    w.show();

    app.run();
}
