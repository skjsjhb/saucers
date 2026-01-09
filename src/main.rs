use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use saucers::app::App;
use saucers::app::AppEventListener;
use saucers::app::AppManager;
use saucers::app::AppOptions;
use saucers::navigation::Navigation;
use saucers::policy::Policy;
use saucers::scheme::register_scheme;
use saucers::scheme::Executor;
use saucers::scheme::Request;
use saucers::scheme::Response;
use saucers::stash::Stash;
use saucers::state::LoadState;
use saucers::status::HandleStatus;
use saucers::webview::ScriptTime;
use saucers::webview::Webview;
use saucers::webview::WebviewEventListener;
use saucers::webview::WebviewOptions;
use saucers::webview::WebviewSchemeHandler;
use saucers::window::Window;
use saucers::NoOp;

fn main() {
    register_scheme("test");

    let app = AppManager::new(AppOptions::new_with_id("hello"));

    let counter = Arc::new(());

    #[derive(Default)]
    struct Trace {
        quit_fired: bool,
        message_received: bool,
        load_fired: bool,
        dom_ready_fired: bool,
        navigate_fired: bool,
        inject_script_executed: bool,
    }

    impl Trace {
        fn verify(&self) {
            assert!(self.quit_fired, "quit event should be fired");
            assert!(self.message_received, "message should be received");
            assert!(self.load_fired, "load event should be fired");
            assert!(self.dom_ready_fired, "DOM ready event should be fired");
            assert!(self.navigate_fired, "navigate event should be fired");
            assert!(self.inject_script_executed, "inject script should be executed");
        }
    }

    #[derive(Clone)]
    struct SharedTrace {
        trace: Rc<RefCell<Trace>>,
        _counter: Arc<()>,
    }

    let trace = Rc::new(RefCell::new(Trace::default()));

    impl AppEventListener for SharedTrace {
        fn on_quit(&self, _app: App) -> Policy {
            self.trace.borrow_mut().quit_fired = true;
            Policy::Allow
        }
    }

    impl WebviewEventListener for SharedTrace {
        fn on_dom_ready(&self, webview: Webview) {
            self.trace.borrow_mut().dom_ready_fired = true;
            webview.execute("window.saucer.internal.message(window._injected.toString());");
        }

        fn on_navigate(&self, _webview: Webview, _nav: &Navigation) -> Policy {
            self.trace.borrow_mut().navigate_fired = true;
            Policy::Allow
        }

        fn on_message(&self, webview: Webview, msg: Cow<str>) -> HandleStatus {
            if msg == "true" {
                self.trace.borrow_mut().inject_script_executed = true;
                webview.window().close();
            } else {
                self.trace.borrow_mut().message_received = true;
            }

            HandleStatus::Handled
        }

        fn on_load(&self, _webview: Webview, _state: LoadState) {
            self.trace.borrow_mut().load_fired = true;
        }
    }

    let trace_app = SharedTrace { trace: trace.clone(), _counter: counter.clone() };
    let trace_webview = trace_app.clone();

    const HTML: &str = r#"
        <script>
            window.saucer.internal.message('Hello');
        </script>
    "#;

    const URL: &str = "test://some/content";

    struct SchemeHd;

    impl WebviewSchemeHandler for SchemeHd {
        fn handle_scheme(&self, _webview: Webview, req: Request, exc: Executor) {
            assert_eq!(req.url().content(), URL, "URL content should be correct");
            exc.accept(Response::new(Stash::new_view(HTML.as_bytes()), "text/html"));
        }
    }

    app.run(
        {
            let counter = counter.clone();
            |app, fin| {
                let wnd = Window::new(&app, NoOp).unwrap();
                wnd.show();

                let schemes = vec!["test".into()];

                let wv =
                    Webview::new(WebviewOptions::default(), wnd, trace_webview, SchemeHd, schemes)
                        .unwrap();

                wv.inject("window._injected = true;", ScriptTime::Creation, true, true);
                wv.set_url_str(URL);

                fin.set(move |_| {
                    drop(wv);
                    drop(counter);
                });
            }
        },
        trace_app,
    )
    .unwrap();

    assert_eq!(Arc::strong_count(&counter), 1, "closures should be dropped");
    trace.borrow().verify();
}
