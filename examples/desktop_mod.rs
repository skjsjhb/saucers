use saucers::app::AppManager;
use saucers::app::AppOptions;
use saucers::desktop::Desktop;
use saucers::desktop::PickerOptions;
use saucers::NoOp;

/// This example shows how to pick a file using the desktop module, then open it with the system
/// default handler.
fn main() {
    let app = AppManager::new(AppOptions::new_with_id("desktop"));

    app.run(
        |app, _| {
            let dsk = Desktop::new(&app);
            match dsk.pick_file(&PickerOptions::new()) {
                Ok(fp) => println!("Selected file path: {fp}"),
                Err(ex) => println!("Did not select file: {ex}"),
            }

            app.quit();
        },
        NoOp,
    )
    .unwrap();
}
