use saucers::app::App;
use saucers::collector::Collector;
use saucers::embed::EmbedFile;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::stash::Stash;
use saucers::webview::Webview;

fn main() {
    let cc = Collector::new();
    let app = App::new(&cc, AppOptions::new("saucer"));
    let w = Webview::new(&Preferences::new(&app)).unwrap();

    let data = "hello, world";
    let stash = Stash::take(data.as_bytes().to_vec());
    let embed = EmbedFile::new(stash, "text/plain");
    w.embed_file("hello", &embed, true);
    w.embed_file("hello-1", &embed, true);
    drop(embed);

    w.set_url("https://saucer.app");
    w.show();
    w.set_size(1152, 648);
    w.set_dev_tools(true);
    w.set_title("Saucers");

    app.run();

    drop(w);
}
