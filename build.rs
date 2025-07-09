use std::path::PathBuf;

fn main() {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let profile = std::env::var("PROFILE").unwrap();
    let is_debug = profile == "debug" || profile == "test";

    let build_static = std::env::var("CARGO_FEATURE_STATIC_LIB").is_ok() && os == "windows";
    let crs_lto = std::env::var("CARGO_FEATURE_CROSS_LTO").is_ok() && build_static;

    let mut conf = cmake::Config::new("saucer-bindings");

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

    if os == "windows" && crs_lto {
        conf.generator("Ninja");
        conf.define("CMAKE_C_COMPILER", "clang-cl");
        conf.define("CMAKE_CXX_COMPILER", "clang-cl");
        conf.define("CMAKE_ASM_COMPILER", "clang-cl");
        conf.define("CMAKE_AR", "llvm-lib");

        if !is_debug {
            conf.define("CMAKE_INTERPROCEDURAL_OPTIMIZATION", "ON");
        }
    }

    let dst = conf.build();

    println!("cargo:rustc-link-search=native={}", dst.display());
    println!("cargo:rustc-link-search=native={}/lib", dst.display());

    if build_static {
        println!("cargo:rustc-link-lib=static=saucer-bindings");
        println!("cargo:rustc-link-lib=static=saucer");

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
        }
    } else {
        println!("cargo:rustc-link-lib=dylib=saucer-bindings");
    }

    println!("cargo:rerun-if-changed=saucer-bindings/CMakeLists.txt");

    let bindings = bindgen::Builder::default()
        .header("saucer-bindings/include/saucer/all.rs.h")
        .clang_args(["-x", "c++"])
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Failed to generate bindings");

    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Failed to emit bindings");
}
