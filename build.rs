//! Phones only: makes sherpa-onnx's shared libraries reachable from the app.
//!
//! iOS: sherpa-onnx is SherpaOnnxC.framework, named `@rpath/SherpaOnnxC.framework/SherpaOnnxC`;
//! `scripts/build-ios.sh` copies it into the app's Frameworks folder, and the app has to look
//! there.
//!
//! Android: sherpa-onnx links as shared libraries, which sherpa-onnx-sys unpacks to
//! `<target dir>/sherpa-onnx-prebuilt/jniLibs/<abi>/` and links by name. `dx` copies a library
//! into the APK only when the linker is given its full path, so this passes the two the app needs
//! by path. They needn't exist yet: the paths are only used when the app is linked, after every
//! build script has run.
use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/Frameworks");
        return;
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") { return; }
    let abi = match env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default().as_str() {
        "aarch64" => "arm64-v8a",
        "x86_64" => "x86_64",
        "arm" => "armeabi-v7a",
        "x86" => "x86",
        other => panic!("No Android ABI for {other}"),
    };
    // OUT_DIR is <target dir>/<triple>/<profile>/build/<crate>/out.
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let triple = env::var("TARGET").expect("TARGET");
    let target_dir = out_dir.ancestors()
        .find(|p| p.file_name().is_some_and(|n| n == triple.as_str()))
        .and_then(|p| p.parent())
        .expect("target directory above OUT_DIR");
    let libs = target_dir.join("sherpa-onnx-prebuilt").join("jniLibs").join(abi);
    for lib in ["libsherpa-onnx-c-api.so", "libonnxruntime.so"] {
        println!("cargo:rustc-link-arg={}", libs.join(lib).display());
    }
}
