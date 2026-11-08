use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let cpp_dir = PathBuf::from(&manifest_dir).join("cpp");
    let whisper_dir = cpp_dir.join("whisper.cpp");

    let ggml_version_header = out_dir.join("ggml-version.h");
    let version_content = r#"
    #ifndef GGML_VERSION_H
    #define GGML_VERSION_H
    #define GGML_VERSION "luna-custom-v0.1"
    #define GGML_COMMIT "unknown"
    #endif
    "#;
    fs::write(&ggml_version_header, version_content).expect("Nie udało się wygenerować ggml-version.h");

    let common_includes = vec![
        out_dir.clone(),
        whisper_dir.join("include"),
        whisper_dir.join("ggml").join("include"),
        whisper_dir.join("ggml").join("src"),
        whisper_dir.join("ggml").join("src").join("ggml-cpu"),
        // NOWE: potrzebne dla plików z arch/x86
        whisper_dir.join("ggml").join("src").join("ggml-cpu").join("arch").join("x86"),
    ];

    // --- KONFIGURACJA DLA C ---
    let mut build_c = cc::Build::new();
    for inc in &common_includes {
        build_c.include(inc);
    }
    build_c
    .define("WHISPER_VERSION", "\"luna-custom-v0.1\"")
    .define("GGML_VERSION", "\"luna-custom-v0.1\"")
    .file(whisper_dir.join("ggml").join("src").join("ggml.c"))
    .file(whisper_dir.join("ggml").join("src").join("ggml-alloc.c"))
    .file(whisper_dir.join("ggml").join("src").join("ggml-quants.c"))
    .file(whisper_dir.join("ggml").join("src").join("ggml-cpu").join("ggml-cpu.c"))
    .file(whisper_dir.join("ggml").join("src").join("ggml-cpu").join("quants.c"))
    // NOWE: kwantyzacja SIMD dla x86 (zawiera ggml_vec_dot_q4_0_q8_0 itp.)
    .file(whisper_dir.join("ggml").join("src").join("ggml-cpu").join("arch").join("x86").join("quants.c"));

    // --- KONFIGURACJA DLA C++ ---
    let mut build_cpp = cc::Build::new();
    build_cpp.cpp(true);
    for inc in &common_includes {
        build_cpp.include(inc);
    }
    build_cpp
    .define("WHISPER_VERSION", "\"luna-custom-v0.1\"")
    .define("GGML_VERSION", "\"luna-custom-v0.1\"")
    .file(cpp_dir.join("luna_stt.cpp"))
    .file(cpp_dir.join("aneforge_stub.cpp"))
    .file(whisper_dir.join("src").join("whisper.cpp"))
    // Backend GGML:
    .file(whisper_dir.join("ggml").join("src").join("ggml-backend.cpp"))
    .file(whisper_dir.join("ggml").join("src").join("ggml-backend-dl.cpp"))
    .file(whisper_dir.join("ggml").join("src").join("ggml-backend-meta.cpp"))
    .file(whisper_dir.join("ggml").join("src").join("ggml-backend-reg.cpp"))
    .file(whisper_dir.join("ggml").join("src").join("ggml-threading.cpp"))
    // CPU:
    .file(whisper_dir.join("ggml").join("src").join("ggml-cpu").join("ggml-cpu.cpp"))
    .file(whisper_dir.join("ggml").join("src").join("ggml-cpu").join("binary-ops.cpp"))
    .file(whisper_dir.join("ggml").join("src").join("ggml-cpu").join("unary-ops.cpp"))
    .file(whisper_dir.join("ggml").join("src").join("ggml-cpu").join("ops.cpp"))
    .file(whisper_dir.join("ggml").join("src").join("ggml-cpu").join("vec.cpp"))
    .file(whisper_dir.join("ggml").join("src").join("ggml-cpu").join("traits.cpp"))
    .file(whisper_dir.join("ggml").join("src").join("ggml-cpu").join("repack.cpp"))
    // NOWE: Tiled matmul
    .file(whisper_dir.join("ggml").join("src").join("ggml-cpu").join("tiled").join("tiled.cpp"))
    .file(whisper_dir.join("ggml").join("src").join("ggml-cpu").join("tiled").join("tiled-kernel.cpp"))
    // NOWE: x86 specyficzne pliki
    .file(whisper_dir.join("ggml").join("src").join("ggml-cpu").join("arch").join("x86").join("repack.cpp"))
    .file(whisper_dir.join("ggml").join("src").join("ggml-cpu").join("arch").join("x86").join("cpu-feats.cpp"));

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    for build in [&mut build_c, &mut build_cpp] {
        build.flag_if_supported("-O3")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-function")
        .flag_if_supported("-D_GNU_SOURCE")
        .flag_if_supported("-pthread");

        if target_arch == "x86_64" {
            build.flag_if_supported("-march=native");
        }
    }

    build_cpp
    .flag_if_supported("-std=c++17")
    .flag_if_supported("-Wno-deprecated-declarations");

    build_c.compile("luna_stt_c");
    build_cpp.compile("luna_stt_cpp");

    println!("cargo:rustc-link-lib=dylib=stdc++");
    println!("cargo:rustc-link-lib=dylib=m");
    println!("cargo:rustc-link-lib=dylib=pthread");
    println!("cargo:rustc-link-lib=dylib=dl");
}
