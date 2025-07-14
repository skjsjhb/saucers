use std::path::PathBuf;

fn main() {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap();
    let profile = std::env::var("PROFILE").unwrap();
    let is_debug = profile == "debug" || profile == "test";

    let is_qt5 = std::env::var("CARGO_FEATURE_QT5").is_ok();
    let is_qt6 = std::env::var("CARGO_FEATURE_QT6").is_ok();

    if is_qt5 && is_qt6 {
        panic!("Only one Qt backend may be specified.");
    }

    let qt_dir = if is_qt6 {
        std::env::var("QT6_DIR").unwrap_or("".to_owned())
    } else if is_qt5 {
        std::env::var("QT5_DIR").unwrap_or("".to_owned())
    } else {
        "".to_owned()
    };

    let build_static = std::env::var("CARGO_FEATURE_STATIC_LIB").is_ok();

    let has_desktop_mod = std::env::var("CARGO_FEATURE_DESKTOP_MOD").is_ok();
    let has_pdf_mod = std::env::var("CARGO_FEATURE_PDF_MOD").is_ok();

    let mut conf = cmake::Config::new("saucer-bindings");

    if has_desktop_mod {
        conf.define("saucer_desktop", "ON");
    }

    if has_pdf_mod {
        conf.define("saucer_pdf", "ON");
    }

    if is_qt5 {
        conf.define("saucer_backend", "Qt5");
    }

    if is_qt6 {
        conf.define("saucer_backend", "Qt6");
    }

    if build_static {
        conf.define("SAUCERS_SHARED_LIB", "OFF");

        if !is_debug {
            conf.define("CMAKE_INTERPROCEDURAL_OPTIMIZATION", "ON");
        }
    }

    fn maybe_forward_env(conf: &mut cmake::Config, envs: &str, cms: &str) {
        if let Ok(ev) = std::env::var(envs) {
            println!("Forwarding env {} to {}", envs, cms);
            conf.define(cms, ev);
        }
    }

    maybe_forward_env(&mut conf, "SAUCERS_CMAKE_C_COMPILER", "CMAKE_C_COMPILER");
    maybe_forward_env(&mut conf, "SAUCERS_CMAKE_CXX_COMPILER", "CMAKE_CXX_COMPILER");
    maybe_forward_env(&mut conf, "SAUCERS_CMAKE_ASM_COMPILER", "CMAKE_ASM_COMPILER");
    maybe_forward_env(&mut conf, "SAUCERS_CMAKE_AR", "CMAKE_AR");

    if let Ok(ev) = std::env::var("SAUCERS_CMAKE_GENERATOR") {
        conf.generator(ev);
    }

    if let Ok(ev) = std::env::var("SAUCERS_CMAKE_GENERATOR_TOOLSET") {
        conf.generator_toolset(ev);
    }

    let dst = conf.build();

    println!("cargo:rustc-link-search=native={}", dst.display());
    println!("cargo:rustc-link-search=native={}/lib", dst.display());

    if build_static {
        println!("cargo:rustc-link-lib=static=saucer-bindings");
        println!("cargo:rustc-link-lib=static=saucer");

        if !qt_dir.is_empty() {
            println!("cargo:rustc-link-search=native={}/lib", qt_dir);
            println!("cargo:rustc-link-search=native={}", qt_dir);
        }

        if has_desktop_mod {
            println!("cargo:rustc-link-lib=static=saucer-bindings-desktop");
            println!("cargo:rustc-link-lib=static=saucer-desktop");
        }

        if has_pdf_mod {
            println!("cargo:rustc-link-lib=static=saucer-bindings-pdf");
            println!("cargo:rustc-link-lib=static=saucer-pdf");
        }

        if is_debug {
            println!("cargo:rustc-link-lib=static=fmtd");
        } else {
            println!("cargo:rustc-link-lib=static=fmt");
        }

        if is_qt5 {
            let libs = vec![
                "Qt5Widgets",
                "Qt5WebChannel",
                "Qt5WebEngineCore",
                "Qt5Network",
                "Qt5WebEngineWidgets",
                "Qt5Core",
                "Qt5Gui",
            ];

            for lib in libs {
                if is_debug && os == "windows" {
                    println!("cargo:rustc-link-lib=dylib={}d", lib);
                } else {
                    println!("cargo:rustc-link-lib=dylib={}", lib);
                }
            }
        }

        if is_qt6 {
            let libs = vec![
                "Qt6Widgets",
                "Qt6WebChannel",
                "Qt6WebEngineCore",
                "Qt6Network",
                "Qt6WebEngineWidgets",
                "Qt6Core",
                "Qt6Gui",
            ];

            for lib in libs {
                if is_debug {
                    println!("cargo:rustc-link-lib=dylib={}d", lib);
                } else {
                    println!("cargo:rustc-link-lib=dylib={}", lib);
                }
            }
        }

        if os == "windows" {
            if !is_qt5 && !is_qt6 {
                if target_env == "msvc" {
                    // The static library can only be linked to MSVC ABI
                    println!("cargo:rustc-link-lib=static=WebView2LoaderStatic");
                } else {
                    // Add WebView2 package to search paths
                    println!(
                        "cargo:rustc-link-search=native={}/build/saucer/nuget/Microsoft.Web.WebView2/build/native/{}",
                        dst.display(),
                        get_windows_arch()
                    );
                    println!("cargo:rustc-link-lib=dylib=WebView2Loader");
                }
            }

            if is_debug {
                println!("cargo:rustc-link-lib=dylib=msvcrtd");
            }

            println!("cargo:rustc-link-lib=dylib=shlwapi");
            println!("cargo:rustc-link-lib=dylib=gdiplus");
            println!("cargo:rustc-link-lib=dylib=user32");
            println!("cargo:rustc-link-lib=dylib=advapi32");
            println!("cargo:rustc-link-lib=dylib=ole32");
            println!("cargo:rustc-link-lib=dylib=shell32");
        }

        if os == "macos" {
            if !is_qt5 && !is_qt6 {
                println!("cargo:rustc-link-lib=framework=WebKit");
                println!("cargo:rustc-link-lib=framework=CoreImage");
            }

            println!("cargo:rustc-link-lib=dylib=c++");
            println!("cargo:rustc-link-lib=framework=Cocoa");
        }

        if os == "linux" {
            if !is_qt5 && !is_qt6 {
                pkg_config::probe_library("gtk4").unwrap();
                pkg_config::probe_library("webkitgtk-6.0").unwrap();
                pkg_config::probe_library("libadwaita-1").unwrap();
            }

            println!("cargo:rustc-link-lib=dylib=stdc++");
        }
    } else {
        println!("cargo:rustc-link-lib=dylib=saucer-bindings");
    }

    println!("cargo:rerun-if-changed=saucer-bindings/CMakeLists.txt");

    let mut header = std::fs::read_to_string("saucer-bindings/include/saucer/all.rs.h").unwrap();

    if has_desktop_mod {
        header.push_str("\n#include \"../modules/desktop/include/saucer/desktop.h\"");
    }

    if has_pdf_mod {
        header.push_str("\n#include \"../modules/pdf/include/saucer/pdf.h\"");
    }

    let bindings = bindgen::Builder::default()
        .header_contents("saucer-bindings/include/saucer/all.rs.h", &header)
        .clang_args(["-x", "c++", "-I./saucer-bindings/include"])
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Failed to generate bindings");

    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Failed to emit bindings");
}

fn get_windows_arch() -> String {
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    match target_arch.as_str() {
        "x86_64" => "x64",
        "x86" => "x86",
        "aarch64" => "arm64",
        _ => panic!("Unsupported Windows architecture: {}", target_arch)
    }
    .to_owned()
}
