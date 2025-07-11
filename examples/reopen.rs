use saucers::app::App;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::webview::events::CloseEvent;
use saucers::webview::Webview;

/// This example shows how to listen for the close event and prevent the default behavior conditionally.
fn main() {
    let (_cc, app) = App::new(AppOptions::new("Reopen"));

    let w = Webview::new(&Preferences::new(&app)).unwrap();

    w.set_size(1152, 648);

    let mut allow_close = false;

    w.on(
        CloseEvent,
        Box::new(move |_| {
            if !allow_close {
                allow_close = true;
                println!("Press again to close the window!");
                false
            } else {
                println!("OK I'm closing.");
                true
            }
        })
    );

    w.set_url("about:blank");
    w.show();

    app.run();
}
