use std::path::PathBuf;

fn main() {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let profile = std::env::var("PROFILE").unwrap();
    let is_debug = profile == "debug" || profile == "test";

    let build_static = std::env::var("CARGO_FEATURE_STATIC_LIB").is_ok();
    let crs_lto = std::env::var("CARGO_FEATURE_CROSS_LTO").is_ok() && build_static;

    let has_desktop_mod = std::env::var("CARGO_FEATURE_DESKTOP_MOD").is_ok();
    let has_pdf_mod = std::env::var("CARGO_FEATURE_PDF_MOD").is_ok();

    let mut conf = cmake::Config::new("saucer-bindings");

    if has_desktop_mod {
        conf.define("saucer_desktop", "ON");
    }

    if has_pdf_mod {
        conf.define("saucer_pdf", "ON");
    }

    if build_static {
        // On Windows, MSVC is required to build static library.
        if os == "windows" {
            let target = std::env::var("CARGO_CFG_TARGET_ENV").unwrap();

            if target != "msvc" {
                panic!("MSVC is required to statically link WebView2.");
            }
        }

        conf.define("SAUCERS_SHARED_LIB", "OFF");
    }

    if crs_lto && !is_debug {
        conf.define("CMAKE_INTERPROCEDURAL_OPTIMIZATION", "ON");
    }

    if crs_lto {
        if os == "windows" {
            // MSVC version of clang-cl generates worse result and requires additional setup
            // Ninja usually comes with cmake and is faster and easier to set these flags
            conf.generator("Ninja");
            conf.define("CMAKE_C_COMPILER", "clang-cl");
            conf.define("CMAKE_CXX_COMPILER", "clang-cl");
            conf.define("CMAKE_ASM_COMPILER", "clang-cl");
            conf.define("CMAKE_AR", "llvm-lib");
        } else {
            // Enforce clang for both macOS and Linux
            conf.define("CMAKE_C_COMPILER", "clang");
            conf.define("CMAKE_CXX_COMPILER", "clang");
            conf.define("CMAKE_ASM_COMPILER", "clang");
            conf.define("CMAKE_AR", "llvm-ar");
        }
    }

    let cmake_profile = conf.get_profile().to_owned();
    let dst = conf.build();

    println!("cargo:rustc-link-search=native={}", dst.display());
    println!("cargo:rustc-link-search=native={}/lib", dst.display());

    if build_static {
        println!("cargo:rustc-link-lib=static=saucer-bindings");
        println!("cargo:rustc-link-lib=static=saucer");

        if has_desktop_mod {
            println!(
                "cargo:rustc-link-search=native={}/build/_deps/saucer-desktop-build/{cmake_profile}",
                dst.display()
            );
            println!(
                "cargo:rustc-link-search=native={}/build/_deps/saucer-desktop-build",
                dst.display()
            );
            println!("cargo:rustc-link-lib=static=saucer-bindings-desktop");
            println!("cargo:rustc-link-lib=static=saucer-desktop");
        }

        if has_pdf_mod {
            println!(
                "cargo:rustc-link-search=native={}/build/_deps/saucer-pdf-build/{cmake_profile}",
                dst.display()
            );
            println!(
                "cargo:rustc-link-search=native={}/build/_deps/saucer-pdf-build",
                dst.display()
            );
            println!("cargo:rustc-link-lib=static=saucer-bindings-pdf");
            println!("cargo:rustc-link-lib=static=saucer-pdf");
        }

        if is_debug {
            println!("cargo:rustc-link-lib=static=fmtd");
        } else {
            println!("cargo:rustc-link-lib=static=fmt");
        }

        if os == "windows" {
            println!("cargo:rustc-link-lib=static=WebView2LoaderStatic");

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
            println!("cargo:rustc-link-lib=dylib=c++");
            println!("cargo:rustc-link-lib=framework=Cocoa");
            println!("cargo:rustc-link-lib=framework=WebKit");
            println!("cargo:rustc-link-lib=framework=CoreImage");
        }

        if os == "linux" {
            println!("cargo:rustc-link-lib=dylib=stdc++");
            pkg_config::probe_library("gtk4").unwrap();
            pkg_config::probe_library("webkitgtk-6.0").unwrap();
            pkg_config::probe_library("libadwaita-1").unwrap();
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
