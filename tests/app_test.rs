extern crate core;

use std::sync::Arc;

use saucers::app::App;
use saucers::options::AppOptions;

#[test]
fn app_test() {
    let app = App::new(AppOptions::new("saucer"));
    let app1 = app.clone();
    drop(app1);
    app.run_once(); // Check that app is only freed when the last owner is dropped

    let ah = app.make_handle();
    let ah1 = ah.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let arc = Arc::new(());
    let arc1 = arc.clone();

    app.pool_submit(move || {
        assert!(!ah1.is_thread_safe(), "Submitted tasks should run on new threads");
        tx.send(()).unwrap()
    });

    assert!(rx.try_recv().is_ok(), "Submitting a task should wait for it to finish");

    let ah2 = ah.clone();
    app.pool_emplace(move || {
        let ah3 = ah2.clone();
        ah2.post(move || {
            let app = ah3.upgrade();
            assert!(app.is_some(), "Posted tasks should run on event thread");
            let app = app.unwrap();
            app.quit();
        });
    });

    let ah4 = ah.clone();
    std::thread::spawn(move || {
        ah4.post(move || {
            let _ = &arc1;
        });
    });

    app.run();
    drop(app);

    assert_eq!(Arc::strong_count(&arc), 1, "Posted closures should be dropped");
}
