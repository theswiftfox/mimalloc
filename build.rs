use std::env;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

// based on: https://github.com/microsoft/mimalloc/blob/v3.7.1/contrib/rust/sys/build.rs

fn var(name: &str) -> String {
    env::var(name).unwrap_or_default()
}

fn feature(name: &str) -> bool {
    env::var_os(format!("CARGO_FEATURE_{}", name.to_uppercase())).is_some()
}

fn exclusive(names: &[&str]) {
    let enabled: Vec<_> = names.iter().filter(|f| feature(f)).collect();
    assert!(
        enabled.len() <= 1,
        "the mimalloc features {enabled:?} are mutually exclusive (the `throughput` and `low_memory` presets cannot be combined either)"
    );
}

struct Target {
    os: String,
    env: String,
    arch: String,
    vendor: String,
    features: String,
    is_native: bool,
}

impl Target {
    fn from_env() -> Self {
        Target {
            os: var("CARGO_CFG_TARGET_OS"),
            env: var("CARGO_CFG_TARGET_ENV"),
            arch: var("CARGO_CFG_TARGET_ARCH"),
            vendor: var("CARGO_CFG_TARGET_VENDOR"),
            features: var("CARGO_CFG_TARGET_FEATURE"),
            is_native: var("HOST") == var("TARGET"),
        }
    }

    fn is_linux_or_android(&self) -> bool {
        self.os == "linux" || self.os == "android"
    }

    fn is_override(&self) -> bool {
        // overriding malloc on Windows needs a DLL, but we only build a static library
        feature("override") && self.os != "windows"
    }
}

struct Compiler {
    is_msvc: bool,
    is_clang: bool,
    is_gnu_like: bool,
}

impl Compiler {
    fn of(build: &cc::Build) -> Self {
        let tool = build.get_compiler();
        let is_msvc = tool.is_like_msvc();
        let is_clang = tool.is_like_clang();
        Compiler {
            is_msvc,
            is_clang,
            is_gnu_like: (tool.is_like_gnu() || is_clang) && !is_msvc,
        }
    }
}

type Defines = Vec<(&'static str, &'static str)>;

fn variant_defines(target: &Target) -> Defines {
    let mut defines = Defines::new();
    // cargo sets `CARGO_CFG_DEBUG_ASSERTIONS` since 1.93; `PROFILE` approximates it before that
    let debug_assertions =
        env::var_os("CARGO_CFG_DEBUG_ASSERTIONS").is_some() || var("PROFILE") == "debug";
    if feature("debug") || (feature("debug_in_debug") && debug_assertions) {
        defines.extend([("MI_DEBUG", "3"), ("MI_SHOW_ERRORS", "1")]);
    } else {
        defines.extend([("MI_DEBUG", "0"), ("NDEBUG", "1")]);
    }
    if feature("secure") {
        defines.push(("MI_SECURE", "4"));
    }
    if target.is_override() {
        defines.push(("MI_MALLOC_OVERRIDE", "1"));
        if target.vendor == "apple" {
            defines.extend([("MI_OSX_ZONE", "1"), ("MI_OSX_INTERPOSE", "1")]);
        }
    }
    if target.env == "musl" {
        defines.push(("MI_LIBC_MUSL", "1"));
    }
    // Rust never uses the C++ `new` API
    defines.push(("MI_NO_CXX_NEW_HANDLER", "1"));
    defines
}

fn cpu_overhead_defines() -> Defines {
    let mut defines = Defines::new();
    if feature("no_stats") {
        defines.push(("MI_STATS", "0"));
    }
    if feature("no_profile") {
        defines.push(("MI_PROFILE", "0"));
    }
    defines
}

fn memory_overhead_defines(target: &Target) -> Defines {
    let mut defines = Defines::new();
    if target.is_linux_or_android() {
        // By default we follow the system THP policy (`MI_NO_THP` only skips mimalloc's own
        // `madvise(MADV_HUGEPAGE)`), and purging may split huge pages (`allow_thp=1`). Opting in
        // (as upstream does by default with `allow_thp=2`) tripled the rss of threads with small heaps
        // and kept the steady-state rss 7-11x higher in our benchmarks.
        // (Android keeps the upstream default of 0.)
        let thp = if feature("no_thp") {
            Some("0")
        } else if feature("thp_full") {
            Some("2")
        } else if feature("thp") || target.os == "linux" {
            Some("1")
        } else {
            None
        };
        if let Some(thp) = thp {
            defines.push(("MI_DEFAULT_ALLOW_THP", thp));
        }
        if !feature("thp") && !feature("thp_full") {
            defines.push(("MI_NO_THP", "1"));
        }
    }
    if feature("small_pages") {
        // 32 KiB arena slices (= small pages) instead of 64 KiB; needs an OS page size of at most 16 KiB
        defines.push(("MI_ARENA_SLICE_SHIFT", "15"));
    }
    if feature("purge_eager") {
        // a delay of 0 purges immediately, but was 2-7x slower in our benchmarks
        defines.extend([
            ("MI_DEFAULT_PURGE_DELAY", "100"),
            ("MI_DEFAULT_ARENA_PURGE_MULT", "1"),
        ]);
    } else if feature("purge_lazy") {
        defines.push(("MI_DEFAULT_PURGE_DELAY", "10000"));
    }
    defines
}

/// Extra C defines from `MIMALLOC_DEFINES`, e.g. `MI_DEFAULT_PURGE_DELAY=250 MI_STATS=0`.
fn user_defines() -> Vec<(String, Option<String>)> {
    var("MIMALLOC_DEFINES")
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|d| !d.is_empty())
        .map(|d| match d.split_once('=') {
            Some((name, value)) => (name.to_string(), Some(value.to_string())),
            None => (d.to_string(), None),
        })
        .collect()
}

/// Applies the defines of the features; `MIMALLOC_DEFINES` take precedence.
fn apply_defines(build: &mut cc::Build, defines: Defines) {
    let user = user_defines();
    for (name, value) in defines {
        if !user.iter().any(|(n, _)| n == name) {
            build.define(name, value);
        }
    }
    for (name, value) in &user {
        build.define(name, value.as_deref());
    }
}

/// Options without a `MI_DEFAULT_*` define in `options.c`; we set their default in a constructor
/// (environment variables like `MIMALLOC_PURGE_DELAY` still take precedence).
const OPTION_DEFAULTS: &[(&str, &str)] = &[
    ("MI_DEFAULT_PURGE_DELAY", "mi_option_purge_delay"),
    ("MI_DEFAULT_ARENA_PURGE_MULT", "mi_option_arena_purge_mult"),
    ("MI_DEFAULT_PURGE_DECOMMITS", "mi_option_purge_decommits"),
];

/// A single translation unit with all of mimalloc and the option defaults (so the constructor
/// is always linked in, and cross-language LTO sees everything at once).
fn write_unit(src: &Path, out: &Path) -> PathBuf {
    let mut c = String::new();
    writeln!(c, "#include \"{}\"", src.join("src/static.c").display()).unwrap();
    let guards: Vec<_> = OPTION_DEFAULTS
        .iter()
        .map(|(d, _)| format!("defined({d})"))
        .collect();
    writeln!(c, "#if {}", guards.join(" || ")).unwrap();
    c.push_str("__attribute__((constructor(101))) static void mi_rs_option_defaults(void) {\n");
    for (define, option) in OPTION_DEFAULTS {
        writeln!(
            c,
            "#ifdef {define}\n  mi_option_set_default({option}, {define});\n#endif"
        )
        .unwrap();
    }
    c.push_str("}\n#endif\n");
    let path = out.join("mimalloc-unit.c");
    std::fs::write(&path, c).expect("cannot write mimalloc-unit.c");
    path
}

/// The compiler flags of mimalloc's `CMakeLists.txt`.
fn compiler_flags(build: &mut cc::Build, target: &Target, compiler: &Compiler) {
    if compiler.is_msvc {
        // use C++ atomics with msvc and clang-cl
        build
            .cpp(true)
            .flag("/TP")
            .flag("/Zc:__cplusplus")
            .std("c++17");
    } else if compiler.is_gnu_like {
        build
            .flag("-fvisibility=hidden")
            .flag("-Wno-unknown-pragmas");
        if compiler.is_clang {
            build.flag("-Wno-static-in-inline");
        }
        if target.is_override() {
            build.flag("-fno-builtin-malloc");
        }
        if target.os != "haiku" {
            let tls = if feature("local_dynamic_tls") || target.env == "musl" {
                "local-dynamic"
            } else {
                "initial-exec"
            };
            build.flag(format!("-ftls-model={tls}"));
        }
    }
}

/// The `-Ctarget-cpu` of the rustflags (`.cargo/config.toml` or `RUSTFLAGS`), if any;
/// ignores `generic`, and `native` when cross-compiling.
fn rust_target_cpu(is_native_build: bool) -> Option<String> {
    let flags = var("CARGO_ENCODED_RUSTFLAGS");
    let mut cpu = None;
    let mut args = flags.split('\x1f');
    while let Some(arg) = args.next() {
        let codegen = match arg {
            "-C" | "--codegen" => args.next().unwrap_or_default(),
            _ => arg
                .strip_prefix("-C")
                .or_else(|| arg.strip_prefix("--codegen="))
                .unwrap_or_default(),
        };
        if let Some(value) = codegen.strip_prefix("target-cpu=") {
            cpu = Some(value.to_string()); // the last one wins
        }
    }
    match cpu.as_deref() {
        None | Some("generic") => None,
        Some("native") if !is_native_build => None,
        _ => cpu,
    }
}

/// Extensions that are mandatory from an aarch64 architecture level on, as reported in
/// `CARGO_CFG_TARGET_FEATURE` (Rust does not report the `v8.Xa` levels themselves).
const AARCH64_LEVELS: &[(&str, &[&str])] = &[
    ("armv8.1-a", &["lse", "rdm"]),
    ("armv8.2-a", &["ras", "dpb"]),
    ("armv8.3-a", &["rcpc", "jsconv", "fcma"]),
    ("armv8.4-a", &["dit", "flagm", "rcpc2"]),
    ("armv8.5-a", &["bti", "sb", "ssbs", "frintts"]),
];

/// The highest aarch64 architecture level above armv8.0 of the Rust target features and the
/// `armv8_*` crate features, as a `-march` value.
fn aarch64_arch_level(target_features: &str) -> Option<&'static str> {
    let has = |f: &&str| target_features.split(',').any(|t| t == *f);
    let from_target = AARCH64_LEVELS
        .iter()
        .take_while(|(_, required)| required.iter().all(has))
        .count();
    let from_features = if feature("armv8_2") {
        2
    } else {
        usize::from(feature("armv8_1"))
    };
    match from_target.max(from_features) {
        // LSE atomics alone (e.g. `-Ctarget-feature=+lse`)
        0 if has(&"lse") => Some("armv8-a+lse"),
        0 => None,
        level => Some(AARCH64_LEVELS[level - 1].0),
    }
}

fn arch_flags(build: &mut cc::Build, target: &Target, compiler: &Compiler) {
    match target.arch.as_str() {
        "aarch64" if compiler.is_gnu_like => {
            // the CPU of `-Ctarget-cpu`, otherwise the highest architecture level of the `armv8_*`
            // features and the Rust target features (if the C compiler supports it)
            let cpu = rust_target_cpu(target.is_native).map(|cpu| format!("-mcpu={cpu}"));
            let arch = aarch64_arch_level(&target.features).map(|arch| format!("-march={arch}"));
            let supported = |flag: &String| {
                let ok = build.is_flag_supported(flag).unwrap_or(false);
                if !ok {
                    println!(
                        "cargo:warning=the C compiler does not support `{flag}`; ignoring it for mimalloc"
                    );
                }
                ok
            };
            if let Some(flag) = cpu.filter(supported).or_else(|| arch.filter(supported)) {
                build.flag(flag);
            } else {
                // armv8.0: use LSE atomics only if the CPU supports them (runtime check)
                build.flag_if_supported("-moutline-atomics");
            }
        }
        "aarch64" if compiler.is_msvc => {
            build.flag(if feature("armv8_2") {
                "/arch:armv8.2"
            } else {
                "/arch:armv8.1"
            });
        }
        "x86_64" if compiler.is_msvc => {
            build.flag("/arch:AVX2");
        }
        _ => {}
    }
}

fn llvm_major(version_output: &str) -> Option<u32> {
    // `rustc -vV`: "LLVM version: 22.1.2", `clang --version`: "clang version 22.1.2 (...)"
    let start = version_output
        .find("LLVM version: ")
        .map(|i| i + "LLVM version: ".len())
        .or_else(|| {
            version_output
                .find("clang version ")
                .map(|i| i + "clang version ".len())
        })?;
    version_output[start..]
        .split('.')
        .next()?
        .trim()
        .parse()
        .ok()
}

fn command_output(cmd: &mut Command) -> String {
    cmd.output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// Emits LLVM bitcode so the linker can inline mimalloc into Rust code (`cross_lang_lto`).
fn cross_lang_lto_flags(build: &mut cc::Build, compiler: &Compiler) {
    assert!(
        compiler.is_clang && !compiler.is_msvc,
        "the `cross_lang_lto` feature needs clang as the C compiler (e.g. CC_{}=clang)",
        var("TARGET").replace('-', "_")
    );
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let rust_llvm = llvm_major(&command_output(Command::new(rustc).arg("-vV")));
    let clang_llvm = llvm_major(&command_output(
        build.get_compiler().to_command().arg("--version"),
    ));
    if rust_llvm != clang_llvm {
        println!(
            "cargo:warning=cross_lang_lto: clang uses LLVM {clang_llvm:?} but rustc uses LLVM {rust_llvm:?}; the LLVM versions must match"
        );
    }
    // must match the Rust side: `lto = "fat"` needs full LTO bitcode, otherwise (thin/off) ThinLTO,
    // or the linker will not inline across the language boundary
    let mode = env::var("MIMALLOC_LTO").unwrap_or_else(|_| "thin".to_string());
    assert!(
        mode == "thin" || mode == "full",
        "MIMALLOC_LTO must be `thin` or `full`"
    );
    build.flag(if mode == "full" {
        "-flto=full"
    } else {
        "-flto=thin"
    });
}

fn link_system_libs(target: &Target) {
    if target.os == "windows" {
        for lib in ["psapi", "shell32", "user32", "advapi32", "bcrypt"] {
            println!("cargo:rustc-link-lib={lib}");
        }
    } else if target.os == "linux" && (target.arch == "arm" || target.arch.starts_with("riscv32")) {
        // Embrace the atomic capability library across various platforms.
        // For instance, on certain platforms, llvm has relocated the atomic of the arm32 architecture to libclang_rt.builtins.a
        // while some use libatomic.a, and others use libatomic_ops.a.
        let atomic_name = env::var("DEP_ATOMIC").unwrap_or_else(|_| "atomic".to_owned());
        println!("cargo:rustc-link-lib={atomic_name}");
    }
}

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let src = manifest.join("mimalloc");
    assert!(
        src.join("src/static.c").exists(),
        "cannot find the mimalloc sources in {} (run `git submodule update --init`)",
        src.display()
    );
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", src.join("include").display());
    println!("cargo:rerun-if-changed={}", src.join("src").display());
    println!("cargo:rerun-if-env-changed=MIMALLOC_DEFINES");
    println!("cargo:rerun-if-env-changed=MIMALLOC_LTO");

    exclusive(&["throughput", "low_memory"]);
    exclusive(&["no_thp", "thp", "thp_full"]);
    exclusive(&["purge_eager", "purge_lazy"]);

    let target = Target::from_env();
    let mut build = cc::Build::new();
    build.include(src.join("include"));
    build.file(write_unit(&src, &out));
    // fails the build if `MiOption` does not match the bundled `mi_option_t`
    build.file(manifest.join("src/option_check.c"));
    println!("cargo:rerun-if-changed=src/option_check.c");
    let compiler = Compiler::of(&build);

    let mut defines = variant_defines(&target);
    defines.extend(cpu_overhead_defines());
    defines.extend(memory_overhead_defines(&target));
    apply_defines(&mut build, defines);
    compiler_flags(&mut build, &target, &compiler);
    arch_flags(&mut build, &target, &compiler);
    if feature("cross_lang_lto") {
        cross_lang_lto_flags(&mut build, &compiler);
    }
    build.warnings(false).compile("mimalloc");

    link_system_libs(&target);
}
