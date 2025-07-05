use saucers::app::App;
use saucers::collector::Collector;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::webview::Webview;

#[test]
fn webview_test() {
    let cc = Collector::new();
    let app = App::new(&cc, AppOptions::new("saucer"));
    let w = Webview::new(&Preferences::new(&app)).unwrap();

    w.set_url("https://saucer.app");
    w.set_size(1152, 648);
    w.set_dev_tools(true);
    w.set_title("Saucers");
    w.show();

    w.on_message({
        let w = w.clone();
        let app = app.clone();
        move |_: &str| -> bool {
            w.close();
            app.quit();
            true
        }
    });

    w.execute("window.saucer.internal.send_message('')");

    app.run();

    w.off_message();

    std::thread::spawn(move || {
        drop(w);
    })
    .join()
    .unwrap();

    drop(app);
    drop(cc);
}
