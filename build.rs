use std::path::Path;
use std::path::PathBuf;

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();

    let dst = cmake::Config::new("saucer-bindings")
        .define("SAUCERS_SHARED_LIB", if target_os == "windows" { "OFF" } else { "ON" })
        .build();

    println!("cargo:rustc-link-search=native={}/lib", dst.display());

    if target_os == "windows" {
        let target = std::env::var("CARGO_CFG_TARGET_ENV").unwrap();

        if target != "msvc" {
            panic!("MSVC is required to link to WebView2");
        }

        let profile = std::env::var("PROFILE").unwrap();
        let is_debug = profile == "debug" || profile == "test";

        let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str() {
            "x86" => "x86",
            "x86_64" => "x64",
            "aarch64" => "arm64",
            it => panic!("Unsupported architecture: {}", it)
        };
        println!(
            "cargo:rustc-link-search=native={}/build/_deps/saucer-build/nuget/Microsoft.Web.WebView2/build/native/{arch}",
            dst.display()
        );
        println!("cargo:rustc-link-lib=static=saucer-bindings");
        println!("cargo:rustc-link-lib=static=saucer");

        if is_debug {
            println!("cargo:rustc-link-lib=static=fmtd");
        } else {
            println!("cargo:rustc-link-lib=static=fmt");
        }

        println!("cargo:rustc-link-lib=static=WebView2LoaderStatic");

        if profile == "debug" || profile == "test" {
            println!("cargo:rustc-link-lib=dylib=msvcrtd");
        }

        println!("cargo:rustc-link-lib=dylib=shlwapi");
        println!("cargo:rustc-link-lib=dylib=gdiplus");
        println!("cargo:rustc-link-lib=dylib=user32");
        println!("cargo:rustc-link-lib=dylib=advapi32");
    } else {
        println!("cargo:rustc-link-lib=dylib=saucer-bindings");
        copy_dylib(&dst);
    }

    println!("cargo:rerun-if-changed=src/lib.rs");
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

fn copy_dylib(dst: &Path) {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();

    let (prefix, suffix) = match target_os.as_str() {
        "windows" => ("", "dll"),
        "macos" => ("lib", "dylib"),
        _ => ("lib", "so")
    };

    let build_profile = std::env::var("PROFILE").unwrap();
    let lib_name = format!("{prefix}saucer-bindings.{suffix}");

    let lib_dir = if target_os == "windows" {
        dst.join("bin")
    } else {
        dst.join("lib")
    };

    let src_lib_path = lib_dir.join(&lib_name);

    let mut dst_paths = Vec::new();

    let exe_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("target")
        .join(&build_profile);

    dst_paths.push(exe_dir.join(&lib_name));
    dst_paths.push(exe_dir.join("deps").join(&lib_name));

    for dst in dst_paths {
        std::fs::create_dir_all(dst.parent().unwrap()).expect("Failed to create destination directory");
        std::fs::copy(&src_lib_path, &dst).expect("Failed to copy shared library");
    }
}
