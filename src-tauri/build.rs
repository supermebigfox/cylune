fn main() {
    println!("cargo:rerun-if-changed=native/mac/bridge.h");
    println!("cargo:rerun-if-changed=native/mac/pet_lifecycle.h");
    println!("cargo:rerun-if-changed=native/mac/pet_lifecycle_test.cc");
    println!("cargo:rerun-if-changed=native/mac/pet.mm");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .cpp(true)
            .file("native/mac/pet.mm")
            .flag("-std=c++17")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .compile("pet_native");
        for framework in [
            "AppKit",
            "Metal",
            "MetalKit",
            "QuartzCore",
            "CoreMedia",
            "CoreVideo",
        ] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
        println!("cargo:rustc-link-arg=-Wl,-weak_framework,ScreenCaptureKit");
    }

    tauri_build::build()
}
