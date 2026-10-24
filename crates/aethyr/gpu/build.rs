//! With the `native` feature, generates bindings from the selected `webgpu.h`
//! implementation and links it. Set `AF_WEBGPU_DIR` to a directory containing
//! `include/webgpu/webgpu.h` (Dawn) or `include/webgpu.h` (wgpu-native) and a
//! `lib` directory with the library. `AF_WEBGPU_IMPL` is `dawn` or `wgpu-native`.

fn main() {
    println!("cargo:rerun-if-env-changed=AF_WEBGPU_DIR");
    println!("cargo:rerun-if-env-changed=AF_WEBGPU_IMPL");
    #[cfg(feature = "native")]
    native::generate();
}

#[cfg(feature = "native")]
mod native {
    use std::{env, path::PathBuf};

    pub fn generate() {
        let target = env::var("TARGET").unwrap();
        if target.starts_with("wasm32") {
            return;
        }
        let dir = PathBuf::from(env::var("AF_WEBGPU_DIR").expect(
            "set AF_WEBGPU_DIR to a Dawn or wgpu-native install (see tools/build-webgpu-native.sh)",
        ));
        let implementation = env::var("AF_WEBGPU_IMPL").unwrap_or_else(|_| "dawn".into());
        let include = dir.join("include");
        let header = match implementation.as_str() {
            "dawn" => include.join("webgpu/webgpu.h"),
            "wgpu-native" => include.join("webgpu.h"),
            other => panic!("AF_WEBGPU_IMPL must be dawn or wgpu-native, not {other}"),
        };
        let mut builder = bindgen::Builder::default()
            .header(header.to_string_lossy())
            .clang_arg(format!("-I{}", include.display()))
            .allowlist_function("wgpu.*")
            .allowlist_type("WGPU.*")
            .allowlist_var("WGPU.*")
            .prepend_enum_name(false)
            .layout_tests(false)
            .derive_default(true)
            .generate_comments(false);
        if implementation == "wgpu-native" {
            // wgpu-native's extensions (device polling) live in wgpu.h.
            builder = builder.header(include.join("wgpu.h").to_string_lossy());
            println!("cargo:rustc-cfg=af_wgpu_native");
        } else {
            println!("cargo:rustc-cfg=af_dawn");
        }
        let bindings = builder.generate().expect("generate webgpu.h bindings");
        let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("webgpu.rs");
        bindings.write_to_file(out).unwrap();

        let lib = dir.join("lib");
        println!("cargo:rustc-link-search=native={}", lib.display());
        println!("cargo:rustc-env=AF_WEBGPU_LIB_DIR={}", lib.display());
        match implementation.as_str() {
            "dawn" => println!("cargo:rustc-link-lib=dylib=webgpu_dawn"),
            _ => println!("cargo:rustc-link-lib=dylib=wgpu_native"),
        }
        if target.contains("apple") || target.contains("linux") {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
        }
    }
}
