use saucers::app::App;
use saucers::app::AppManager;
use saucers::app::AppOptions;
use saucers::app::FinishListener;
use saucers::scheme::register_scheme;
use saucers::webview::Webview;
use saucers::webview::WebviewOptions;
use saucers::window::Window;
use saucers::NoOp;

fn start(app: App, fin: &mut FinishListener) {
    let wnd = Window::new(&app, NoOp).unwrap();

    let opt = WebviewOptions::default();

    let wv = Webview::new(opt, wnd.clone(), NoOp, NoOp, vec!["test".into()]).unwrap();

    wv.set_dev_tools(true);
    wnd.set_title("Hello");
    wnd.show();

    fin.set(move |_| {
        drop(wv);
        drop(wnd);
    });
}

fn main() {
    register_scheme("test");

    let app = AppManager::new(AppOptions::new_with_id("hello"));

    app.run(start, NoOp).unwrap();
}
