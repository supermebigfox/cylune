fn main() {
    println!("cargo:rerun-if-changed=native/mac/bridge.h");
    println!("cargo:rerun-if-changed=native/mac/pet_lifecycle.h");
    println!("cargo:rerun-if-changed=native/mac/pet_lifecycle_test.cc");
    println!("cargo:rerun-if-changed=native/mac/pet_drop_state.h");
    println!("cargo:rerun-if-changed=native/mac/pet_position.h");
    println!("cargo:rerun-if-changed=native/mac/pet_position_test.cc");
    println!("cargo:rerun-if-changed=native/mac/pet.mm");
    println!("cargo:rerun-if-changed=native/mac/tiyda/BlackHoleDesktop.h");
    println!("cargo:rerun-if-changed=native/mac/tiyda/MetalBlackHoleView.m");
    println!("cargo:rerun-if-changed=native/mac/tiyda/BlackHole.metal");
    println!("cargo:rerun-if-changed=native/mac/tiyda/black_hole_params.h");
    println!("cargo:rerun-if-changed=native/mac/tiyda/capture_policy.h");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .cpp(true)
            .file("native/mac/pet.mm")
            .flag("-std=c++17")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .compile("pet_native");
        cc::Build::new()
            .file("native/mac/tiyda/MetalBlackHoleView.m")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .compile("tiyda_black_hole");
        for framework in ["AppKit", "Metal", "MetalKit", "QuartzCore"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
        println!("cargo:rustc-link-arg=-Wl,-weak_framework,ScreenCaptureKit");
    }

    tauri_build::build()
}
