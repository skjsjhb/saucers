use saucers::app::App;
use saucers::embed::EmbedFile;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::stash::Stash;
use saucers::webview::Webview;

// Files can be included in the binary using macros.
// This is usually automated by frameworks in real-world applications.
static HTML_FILE: &[u8] = include_bytes!("index.html");
static JS_FILE: &[u8] = include_bytes!("main.js");

/// This example shows how to embed, serve and use resource files.
fn main() {
    // Create embeddable files using borrowed stashes to avoid data copying.
    let html_file = EmbedFile::new(&Stash::view(HTML_FILE), "text/html");
    let js_file = EmbedFile::new(&Stash::view(JS_FILE), "application/javascript");

    let (_cc, app) = App::new(AppOptions::new("EmbedFile"));

    let w = Webview::new(&Preferences::new(&app)).unwrap();
    w.set_size(1152, 648);

    // Configure the files to be served.
    w.embed_file("index.html", &html_file, false);
    w.embed_file("main.js", &js_file, false);

    // Navigates to the embedded file.
    w.serve("index.html");

    w.show();

    app.run();
}
