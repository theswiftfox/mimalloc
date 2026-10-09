use std::env;

// source: https://github.com/purpleprotocol/mimalloc_rust/blob/master/libmimalloc-sys/build.rs

fn main() {
    let mut build = cc::Build::new();

    build.include("mimalloc/include");
    build.include("mimalloc/src");
    build.file("mimalloc/src/static.c");
    // Fails the build if `MiOption` does not match the bundled `mi_option_t`.
    build.file("src/option_check.c");
    println!("cargo:rerun-if-changed=src/option_check.c");
    println!("cargo:rerun-if-changed=mimalloc/include/mimalloc.h");

    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("target_os not defined!");
    let target_family = env::var("CARGO_CFG_TARGET_FAMILY").expect("target_family not defined!");
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("target_arch not defined!");
    let build_civic = match env::var("BUILD_CIVIC") {
        Ok(val) => val == "1",
        Err(_) => false,
    };

    if env::var_os("CARGO_FEATURE_OVERRIDE").is_some() {
        // Overriding malloc is only available on windows in shared mode, but we
        // only ever build a static lib.
        if target_family != "windows" {
            build.define("MI_MALLOC_OVERRIDE", None);
        }
    }

    if env::var_os("CARGO_FEATURE_SECURE").is_some() {
        build.define("MI_SECURE", "4");
    }

    let dynamic_tls = env::var("CARGO_FEATURE_LOCAL_DYNAMIC_TLS").is_ok();

    if target_family == "unix" && target_os != "haiku" {
        if dynamic_tls {
            build.flag_if_supported("-ftls-model=local-dynamic");
        } else {
            build.flag_if_supported("-ftls-model=initial-exec");
        }
    }

    if (target_os == "linux" || target_os == "android" || target_os == "macos")
        && env::var_os("CARGO_FEATURE_NO_THP").is_some()
    {
        build.define("MI_NO_THP", "1");
    }

    if target_arch == "aarch64" {
        // build.define("MI_OPT_SIMD", "1");
        if build_civic {
            build.flag_if_supported("-march=armv8.2-a");
        }
        build.flag_if_supported("-march=armv8-a");
    }

    if target_os == "windows" {
        build.flag("/TP");
        build.flag("/DWIN32");
        build.flag("/D_WINDOWS");
        build.flag("-std:c++17");
        build.flag("/Zc:__cplusplus");
        if target_arch == "x86_64" {
            build.flag("/arch:AVX2");
        } else if target_arch == "aarch64" {
            build.flag("/arch:armv8.1");
        }
    }

    // smaller slices to reduce RSS
    build.define("MI_ARENA_SLICE_SHIFT", "(12 + MI_SIZE_SHIFT)");

    if env::var_os("CARGO_FEATURE_DEBUG").is_some()
        || (env::var_os("CARGO_FEATURE_DEBUG_IN_DEBUG").is_some() && cfg!(debug_assertions))
    {
        build.define("MI_DEBUG", "3");
        build.define("MI_SHOW_ERRORS", "1");
    } else {
        // Remove heavy debug assertions etc
        build.define("MI_DEBUG", "0");
    }

    if build.get_compiler().is_like_msvc() {
        build.cpp(true);
    }

    build.compile("mimalloc");

    // on armv6 we need to link with libatomic
    if target_os == "linux" && target_arch == "arm" {
        // Embrace the atomic capability library across various platforms.
        // For instance, on certain platforms, llvm has relocated the atomic of the arm32 architecture to libclang_rt.builtins.a
        // while some use libatomic.a, and others use libatomic_ops.a.
        let atomic_name = env::var("DEP_ATOMIC").unwrap_or("atomic".to_owned());
        println!("cargo:rustc-link-lib={atomic_name}");
    }
}
