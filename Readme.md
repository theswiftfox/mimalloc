# Mimalloc

Rust bindings for [mimalloc](https://github.com/microsoft/mimalloc) version 3.7.1,
tunable for throughput or memory usage with crate features.

```rust
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
```

## Features

Without features you get a balanced build: the upstream defaults, except that transparent huge
pages (THP) follow the system policy instead of being requested by mimalloc, and may be split when
purging. This keeps the rss close to the previous v3.0.1 builds (see [benchmarks](#benchmarks)).

| Preset | Enables | Use for |
|---|---|---|
| `throughput` | `no_stats`, `no_profile`, `thp_full`, `purge_lazy` | CPU-bound services where memory is plentiful |
| `low_memory` | `no_thp`, `small_pages`, `purge_eager` | memory-constrained targets |

The presets cannot be combined, but you can combine individual features.

| Feature | Effect | Trade-off |
|---|---|---|
| `armv8_1` / `armv8_2` | inline LSE atomics (`-march=armv8.1-a` / `armv8.2-a`) instead of runtime dispatched atomics | needs an armv8.1+ CPU (not Cortex-A53/A57/A72); +35-50% on cross-thread frees |
| `no_stats` | no detailed statistics | `stats_print` shows only the essential counters |
| `no_profile` | compiles out the `MIMALLOC_PROFILE` sampling heap profiler | |
| `cross_lang_lto` | inlines the mimalloc fast paths into Rust code | needs clang + lld matching rustc's LLVM, see below; +3-5% |
| `no_thp` | disables THP for the whole process (`PR_SET_THP_DISABLE`, also for non-mimalloc memory) | lower rss, more TLB misses |
| `thp` | requests THP (`MADV_HUGEPAGE`) even if the system policy is `madvise` | fewer TLB misses (+30% on random-access workloads), up to 3x rss with many threads |
| `thp_full` | as `thp`, and never split huge pages when purging (upstream default) | as `thp`, and 7-11x higher steady-state rss |
| `small_pages` | 32 KiB instead of 64 KiB small pages | ~6% lower steady rss, 10-20% slower; needs an OS page size of at most 16 KiB |
| `purge_eager` | return free memory after 100 ms (instead of 1 s, 4 s for arenas) | memory returns ~4x sooner, ~20% slower for bursty allocations |
| `purge_lazy` | return free memory after 10 s (40 s for arenas) | fewer `madvise` calls and page faults, memory held much longer |
| `override` | also replaces `malloc`/`free` in the final binary | |
| `secure` | full security mitigations (`MI_SECURE=4`) | slower and more memory |
| `local_dynamic_tls` | TLS model for allocators in `dlopen`ed libraries | slightly slower |
| `debug` / `debug_in_debug` | mimalloc assertions and invariant checks (always / with `debug_assertions`) | very slow |

`no_thp`/`thp`/`thp_full` and `purge_eager`/`purge_lazy` are mutually exclusive.

### Target CPU

mimalloc is compiled for the same CPU as your Rust code. If you set a Rust target CPU, e.g.

```toml
# .cargo/config.toml
[target.aarch64-unknown-linux-gnu]
rustflags = ["-Ctarget-cpu=cortex-a78"]
```

mimalloc is compiled with `-mcpu=cortex-a78`. If the C compiler does not know the CPU name, the
architecture level of the Rust target features is used instead (e.g. `-march=armv8.2-a`).
Without a target CPU, the `armv8_1`/`armv8_2` features select the architecture level, and
otherwise the library is built for armv8.0 and runs on every aarch64 CPU.

### Runtime overrides

The features only set defaults. The `MIMALLOC_*` environment variables
(e.g. `MIMALLOC_PURGE_DELAY=0`, `MIMALLOC_ALLOW_THP=0`, `MIMALLOC_SHOW_STATS=1`)
take precedence, and so does `mimalloc::options::option_set` at runtime.
`mimalloc::options::collect(true)` returns all free memory to the OS right away, e.g. after a load peak.

### Build overrides

`MIMALLOC_DEFINES` passes extra C defines to the mimalloc build and takes precedence over the
features, e.g. `MIMALLOC_DEFINES="MI_DEFAULT_PURGE_DELAY=0 MI_DEFAULT_ARENA_PURGE_MULT=1"`.
Besides the `MI_DEFAULT_*` option defaults of mimalloc's `options.c`, this crate supports
`MI_DEFAULT_PURGE_DELAY`, `MI_DEFAULT_ARENA_PURGE_MULT` and `MI_DEFAULT_PURGE_DECOMMITS`.

### Cross-language LTO

`cross_lang_lto` compiles mimalloc to LLVM bitcode so the linker can inline `malloc`/`free`
fast paths into Rust code. It needs:

- clang as the C compiler, with the same LLVM major version as `rustc -vV` (LLVM 22 for Rust 1.95),
- lld with the same LLVM version, and
- the same LTO mode on both sides: `MIMALLOC_LTO=full` with `lto = "fat"`, otherwise the default `thin`.

```sh
CC=clang-22 MIMALLOC_LTO=full \
RUSTFLAGS="-Clinker-plugin-lto -Clinker=clang-22 -Clink-arg=-fuse-ld=lld" \
cargo build --release --features mimalloc/cross_lang_lto
```

When cross-compiling, use the target-specific variables (`CC_aarch64_unknown_linux_gnu`, and pass
`--sysroot` in `CFLAGS_aarch64_unknown_linux_gnu`). If the LLVM versions or LTO modes differ, the
build still works, but nothing gets inlined.

## Benchmarks

`bench/` contains the benchmark used to tune this crate (`cargo run --release` in `bench/`, or
`--features mimalloc/throughput` etc.). The numbers below are from a Fedora 40 aarch64 VM
(Parallels on an Apple M3 Pro, 4 KiB pages, THP policy `madvise`, 4 threads, best of 5).

> **Note:** in Linux VMs on Apple Silicon (both Parallels and Docker Desktop), sequential stores to
> THP-backed memory are 5-7x slower, even for plain `mmap` memory without any allocator, while
> random-access workloads get ~30% faster with THP. So THP results from these VMs do not carry over
> to other hardware. The CPU table was measured with THP disabled. Measure THP on the real target.

Throughput in Mops/s (higher is better). `old` is the previous v3.0.1 build of this crate.

| Workload | old | default | `armv8_2` | `throughput,armv8_2` | `low_memory` | glibc |
|---|---|---|---|---|---|---|
| alloc+free 32 B | 170 | 424 | 440 | 425 | 436 | 163 |
| random churn, 1 thread | 43.0 | 54.7 | 55.2 | 56.2 | 48.5 | 37.6 |
| random churn, 4 threads | 137 | 175 | 178 | 184 | 161 | 134 |
| zeroed | 46.5 | 54.9 | 55.3 | 55.3 | 55.3 | 28.5 |
| vec growth (realloc) | 2201 | 2189 | 2191 | 2193 | 2211 | 1937 |
| string map | 13.6 | 13.0 | 13.2 | 13.0 | 12.9 | 8.9 |
| cross-thread free | 38.0 | 54.0 | 74.1 | 68.5 | 55.6 | 16.7 |

With THP requested (`thp`), the random churn got 30-40% faster in this VM, while
alloc+free 32 B was 32% slower and vec growth 5x slower (see the note above).

Rss in MiB when 4 threads build a 270 MiB working set, then free 90% and keep running:

| | peak | 1 s after | 4.5 s after |
|---|---|---|---|
| old | 271 | 271 | 10.8 |
| default | 272 | 272 | 21 (purged at ~4 s) |
| `thp` | 330 | 330 | 29 |
| `throughput` | 330 | 330 | 330 (purged after 40 s) |
| `low_memory` | 273 | 15-23 | 11.6 |
| glibc | 264 | 124 | 108 |

Rss with 64 threads holding 25 MiB of small objects: old 35.6 MiB, default 38.8 MiB,
`thp`/`throughput` 119 MiB, `low_memory` 39.2 MiB, glibc 26.6 MiB.
On a kernel with THP policy `always`, `thp_full` (the upstream default) kept 136-148 MiB after 4.5 s.
