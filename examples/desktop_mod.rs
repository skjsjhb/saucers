use saucers::app::App;
use saucers::desktop::Desktop;
use saucers::desktop::PickerOptions;
use saucers::options::AppOptions;

/// This example shows how to pick a file using the desktop module, then open it with the system default handler.
fn main() {
    #[cfg(feature = "desktop-mod")]
    {
        let (_cc, app) = App::new(AppOptions::new("DesktopMod"));
        let dsk = Desktop::new(&app);
        let fp = dsk.pick_file(&PickerOptions::new());

        match fp {
            Some(f) => {
                println!("File selected: {}", f);
                dsk.open(f);
            }

            None => {
                println!("No file selected!");
            }
        }
    }
}
