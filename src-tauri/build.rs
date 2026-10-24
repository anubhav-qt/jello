// Runs before the Rust core compiles: builds the C++ engine and links it in,
// then does Tauri's own build step.

fn main() {
    build_engine();
    tauri_build::build();
}

fn build_engine() {
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .include("../engine/include")
        .include("../engine/third_party")
        .file("../engine/src/engine.cpp")
        .file("../engine/src/envelope.cpp")
        .file("../engine/src/filter.cpp")
        .file("../engine/src/wave.cpp")
        .file("../engine/src/oscillator.cpp")
        .file("../engine/src/noise.cpp")
        .file("../engine/src/lfo.cpp")
        .file("../engine/src/routing.cpp")
        .file("../engine/src/sample_set.cpp")
        .file("../engine/src/voice.cpp")
        .file("../engine/src/drum_kit.cpp")
        .file("../engine/src/synth.cpp")
        .file("../engine/src/clock.cpp")
        .file("../engine/src/click.cpp")
        .file("../engine/src/looper.cpp")
        .file("../engine/src/snap.cpp")
        .file("../engine/src/song_player.cpp")
        .file("../engine/src/conductor.cpp")
        .file("../engine/src/capture.cpp")
        .file("../engine/src/miniaudio.cpp")
        .compile("jello_engine");

    // Build the engine again whenever any of its files change.
    println!("cargo:rerun-if-changed=../engine");

    // On macOS, miniaudio talks to the speakers through Apple's audio
    // frameworks (see engine/src/miniaudio_settings.h). Windows needs nothing.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    if target_os == "macos" {
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=CoreAudio");
        println!("cargo:rustc-link-lib=framework=AudioToolbox");
    }
}
