use build_target::Os;

fn main() {
    let os = build_target::target_os();

    let profile = std::env::var("PROFILE").unwrap();
    let is_debug = profile == "debug" || profile == "test";

    if os == Os::Windows && is_debug {
        // Someone added a directive to link to the release library, even in debug mode...
        // I'm looking at you CC 👀
        println!("cargo::rustc-link-arg=/NODEFAULTLIB:msvcrt");
    }
}
