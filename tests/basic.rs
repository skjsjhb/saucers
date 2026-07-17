use std::borrow::Cow;
use std::cell::RefCell;
use std::panic::AssertUnwindSafe;
use std::rc::Rc;
use std::sync::Arc;

use libtest_mimic::Arguments;
use libtest_mimic::Trial;
use saucers::NoOp;
use saucers::app::App;
use saucers::app::AppEventListener;
use saucers::app::AppManager;
use saucers::app::AppOptions;
use saucers::navigation::Navigation;
use saucers::policy::Policy;
use saucers::scheme::Executor;
use saucers::scheme::Request;
use saucers::scheme::Response;
use saucers::scheme::register_scheme;
use saucers::stash::Stash;
use saucers::state::LoadState;
use saucers::status::HandleStatus;
use saucers::webview::ScriptTime;
use saucers::webview::Webview;
use saucers::webview::WebviewEventListener;
use saucers::webview::WebviewOptions;
use saucers::webview::WebviewSchemeHandler;
use saucers::window::Window;

fn main() {
    let mut args = Arguments::from_args();

    args.test_threads = Some(1);

    let tests = vec![Trial::test("app_lifecycle", || {
        app_lifecycle();
        Ok(())
    })];

    libtest_mimic::run(&args, tests).exit();
}

fn app_lifecycle() {
    register_scheme("test");

    let app = AppManager::new(AppOptions::new_with_id("test"));

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
            assert!(
                self.inject_script_executed,
                "inject script should be executed"
            );
        }
    }

    struct SharedTrace(AssertUnwindSafe<Rc<RefCell<Trace>>>); // If `borrow_mut` panics, the event listener is unsound!

    impl Clone for SharedTrace {
        fn clone(&self) -> Self { SharedTrace(AssertUnwindSafe(self.0.clone())) }
    }

    let trace = SharedTrace(AssertUnwindSafe(Rc::new(RefCell::new(Trace::default()))));

    impl AppEventListener for SharedTrace {
        fn on_quit(&self, _app: App) -> Policy {
            self.0.borrow_mut().quit_fired = true;
            Policy::Allow
        }
    }

    impl WebviewEventListener for SharedTrace {
        fn on_dom_ready(&self, webview: Webview) {
            self.0.borrow_mut().dom_ready_fired = true;
            webview.execute("window.saucer.internal.message(window._injected.toString());");
        }

        fn on_navigate(&self, _webview: Webview, _nav: &Navigation) -> Policy {
            self.0.borrow_mut().navigate_fired = true;
            Policy::Allow
        }

        fn on_message(&self, webview: Webview, msg: Cow<str>) -> HandleStatus {
            if msg == "true" {
                self.0.borrow_mut().inject_script_executed = true;
                webview.window().close();
            } else {
                self.0.borrow_mut().message_received = true;
            }

            HandleStatus::Handled
        }

        fn on_load(&self, _webview: Webview, _state: LoadState) {
            self.0.borrow_mut().load_fired = true;
        }
    }

    let trace_app = trace.clone();
    let trace_webview = trace.clone();

    const PAGE_HTML: &str = r#"
        <script>
            window.saucer.internal.message('Hello');
        </script>
    "#;

    const SCHEME_URL: &str = "test://some/content";

    struct SchemeHd;

    impl WebviewSchemeHandler for SchemeHd {
        fn schemes(&self) -> Vec<Cow<'static, str>> { vec!["test".into()] }

        fn handle_scheme(&self, _webview: Webview, req: Request, exc: Executor) {
            assert_eq!(
                req.url().content(),
                SCHEME_URL,
                "URL content should be correct"
            );
            exc.accept(Response::new(
                Stash::new_view(PAGE_HTML.as_bytes()),
                "text/html",
            ));
        }
    }

    let counter = Arc::new(());

    app.run(
        {
            let counter = counter.clone();
            |app, fin| {
                let wnd = Window::new(&app, NoOp).unwrap();
                wnd.show();

                let wv =
                    Webview::new(WebviewOptions::default(), wnd, trace_webview, SchemeHd).unwrap();

                wv.inject("window._injected = true;", ScriptTime::Creation, true, true);
                wv.set_url_str(SCHEME_URL);

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
    trace.0.borrow().verify();
}
