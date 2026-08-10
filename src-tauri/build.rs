fn main() {
    println!("cargo:rerun-if-changed=native/mac/bridge.h");
    println!("cargo:rerun-if-changed=native/mac/pet_lifecycle.h");
    println!("cargo:rerun-if-changed=native/mac/pet_lifecycle_test.cc");
    println!("cargo:rerun-if-changed=native/mac/pet_drop_state.h");
    println!("cargo:rerun-if-changed=native/mac/pet_ingest_animation.h");
    println!("cargo:rerun-if-changed=native/mac/pet_position.h");
    println!("cargo:rerun-if-changed=native/mac/pet_position_test.cc");
    println!("cargo:rerun-if-changed=native/mac/pet.mm");
    println!("cargo:rerun-if-changed=native/mac/tiyda/BlackHoleDesktop.h");
    println!("cargo:rerun-if-changed=native/mac/tiyda/MetalBlackHoleView.m");
    println!("cargo:rerun-if-changed=native/mac/tiyda/BlackHole.metal");
    println!("cargo:rerun-if-changed=native/mac/tiyda/black_hole_params.h");
    println!("cargo:rerun-if-changed=native/mac/tiyda/capture_policy.h");
    println!("cargo:rerun-if-changed=native/windows/bridge.h");
    println!("cargo:rerun-if-changed=native/windows/pet_bridge.cpp");
    println!("cargo:rerun-if-changed=native/windows/callback_guard.h");
    println!("cargo:rerun-if-changed=native/windows/callback_guard_test.cc");
    println!("cargo:rerun-if-changed=native/windows/drop_target.h");
    println!("cargo:rerun-if-changed=native/windows/drop_target.cpp");
    println!("cargo:rerun-if-changed=native/windows/drop_target_test.cc");
    println!("cargo:rerun-if-changed=native/windows/drop_state.h");
    println!("cargo:rerun-if-changed=native/windows/drop_state_test.cc");
    println!("cargo:rerun-if-changed=native/windows/window.h");
    println!("cargo:rerun-if-changed=native/windows/window.cpp");
    println!("cargo:rerun-if-changed=native/windows/window_state.h");
    println!("cargo:rerun-if-changed=native/windows/window_state_test.cc");
    println!("cargo:rerun-if-changed=native/windows/renderer.h");
    println!("cargo:rerun-if-changed=native/windows/renderer.cpp");
    println!("cargo:rerun-if-changed=native/windows/render_state.h");
    println!("cargo:rerun-if-changed=native/windows/render_state_test.cc");
    println!("cargo:rerun-if-changed=native/windows/BlackHole.hlsl");

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

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        cc::Build::new()
            .cpp(true)
            .std("c++17")
            .file("native/windows/pet_bridge.cpp")
            .file("native/windows/drop_target.cpp")
            .file("native/windows/renderer.cpp")
            .file("native/windows/window.cpp")
            .compile("pet_native_windows");
        for library in [
            "user32",
            "gdi32",
            "ole32",
            "uuid",
            "shell32",
            "d3d11",
            "dxgi",
            "dcomp",
            "dwmapi",
            "d3dcompiler",
        ] {
            println!("cargo:rustc-link-lib={library}");
        }
    }

    tauri_build::build()
}
