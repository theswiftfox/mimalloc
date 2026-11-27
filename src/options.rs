//! Runtime options and maintenance functions of mimalloc.
//!
//! Options can also be set through `MIMALLOC_<OPTION>` environment variables, see the
//! [mimalloc documentation](https://microsoft.github.io/mimalloc/environment.html).
//! The crate features change some option defaults (see the readme); environment variables and
//! [`option_set`] take precedence over those.
//! Some options (e.g. [`MiOption::ArenaEagerCommit`] or [`MiOption::AllowThp`]) are only read when
//! the process starts or the first arena is created, so setting them later has no effect.

use core::ffi::c_long;

mod ffi {
    use core::ffi::{c_int, c_long};

    unsafe extern "C" {
        pub(crate) fn mi_option_is_enabled(option: c_int) -> bool;
        pub(crate) fn mi_option_set_enabled(option: c_int, enable: bool);
        pub(crate) fn mi_option_set_enabled_default(option: c_int, enable: bool);
        pub(crate) fn mi_option_get(option: c_int) -> c_long;
        pub(crate) fn mi_option_set(option: c_int, value: c_long);
        pub(crate) fn mi_option_set_default(option: c_int, value: c_long);
        pub(crate) fn mi_collect(force: bool);
    }
}

/// A mimalloc runtime option, mirroring `mi_option_t` of the bundled mimalloc version.
///
/// The discriminants must match the C enum; this is verified at compile time by
/// `src/option_check.c`. Deprecated options are omitted. Sizes are in KiB unless noted.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MiOption {
    /// Print error messages.
    ShowErrors = 0,
    /// Print statistics on termination.
    ShowStats = 1,
    /// Print verbose messages.
    Verbose = 2,
    /// Eagerly commit arenas; 2 enables it only on overcommit systems (=2).
    ArenaEagerCommit = 4,
    /// Decommit memory on purge (1, `MADV_DONTNEED`) or use memory reset instead (0, `MADV_FREE`).
    PurgeDecommits = 5,
    /// Allow large (2 or 4 MiB) OS pages.
    AllowLargeOsPages = 6,
    /// Reserve N huge (1 GiB) OS pages at startup.
    ReserveHugeOsPages = 7,
    /// Reserve huge OS pages at a specific NUMA node.
    ReserveHugeOsPagesAt = 8,
    /// Reserve the given amount of OS memory in an arena at startup.
    ReserveOsMemory = 9,
    /// Delay purging of unused memory by N milliseconds; 0 purges immediately, -1 never (=1000).
    PurgeDelay = 15,
    /// Use at most N NUMA nodes; 0 uses all available nodes.
    UseNumaNodes = 16,
    /// Do not use OS memory for allocation, only programmatically reserved arenas.
    DisallowOsAlloc = 17,
    /// Tag used for OS logging (macOS only).
    OsTag = 18,
    /// Issue at most N error messages.
    MaxErrors = 19,
    /// Issue at most N warning messages.
    MaxWarnings = 20,
    /// Release all memory on exit.
    DestroyOnExit = 22,
    /// Initial memory size for arena reservation (=1 GiB on 64-bit).
    ArenaReserve = 23,
    /// Multiplier of `PurgeDelay` for purging arenas (=4).
    ArenaPurgeMult = 24,
    /// Do not use arenas for allocation (except with specific arena ids).
    DisallowArenaAlloc = 26,
    /// Retry on out-of-memory for N milliseconds (Windows only).
    RetryOnOom = 27,
    /// Minimal rounded object size for guarded objects (requires `MI_GUARDED`).
    GuardedMin = 29,
    /// Maximal rounded object size for guarded objects (requires `MI_GUARDED`).
    GuardedMax = 30,
    /// Guard an object in the min/max range about every N requested bytes.
    GuardedSampleRate = 32,
    /// Seed for the guarded sampling.
    GuardedSampleSeed = 33,
    /// Collect thread-local heaps every N generic allocation calls (=10000).
    GenericCollect = 34,
    /// Reclaim abandoned pages on free: -1 never, 0 into the originating heap only, 1 across heaps.
    PageReclaimOnFree = 35,
    /// Retain N full pages per size class (=2).
    PageFullRetain = 36,
    /// Max. number of candidate pages to consider for allocation (=4).
    PageMaxCandidates = 37,
    /// Max. user space virtual address bits to consider.
    MaxVabits = 38,
    /// Commit the full pagemap.
    PagemapCommit = 39,
    /// Commit page memory on demand.
    PageCommitOnDemand = 40,
    /// Do not reclaim pages of the originating heap if it already owns N pages of that size class (=-1, unlimited).
    PageMaxReclaim = 41,
    /// Do not reclaim pages across threads if the heap already owns N pages of that size class (=32).
    PageCrossThreadMaxReclaim = 42,
    /// Transparent huge pages: 0 disabled for the process, 1 allowed, 2 allowed and never split on purge.
    AllowThp = 43,
    /// Minimal purge size; 0 selects 64 KiB, or 2 MiB with `AllowThp` 2.
    ArenaPurgeMinSize = 44,
    /// Maximal object size allocated in an arena (=2 GiB on 64-bit).
    ArenaMaxObjectSize = 45,
    /// Experimental: associate the local NUMA node with an initial arena allocation.
    ArenaIsNumaLocal = 46,
    /// Merge thread-local statistics into the heap on each collection.
    StatsMergeOnCollect = 47,
    /// Bytes allocated between automatic heap profiler snapshots (=1 GiB).
    ProfileAllocInterval = 48,
    /// Bytes of in-use growth between automatic heap profiler snapshots (=0, disabled).
    ProfileInuseInterval = 49,
    /// Seconds between automatic heap profiler snapshots (=0, disabled).
    ProfileTimeInterval = 50,
    /// Sample rate in bytes of the heap profiler (=512 KiB).
    ProfileSampleRate = 51,
    /// Merge thread-local statistics after N bytes were allocated or freed; 0 disables (=4 MiB).
    StatsMergeThreshold = 52,
    /// Purge freed ranges above this size immediately (=32 MiB).
    ArenaPurgeImmediateSize = 53,
    /// Use memory tagging (MTE) if available and enabled at build time.
    Mte = 54,
}

impl MiOption {
    #[inline]
    fn raw(self) -> core::ffi::c_int {
        self as core::ffi::c_int
    }
}

/// Returns the current value of `option`.
#[must_use]
pub fn option_get(option: MiOption) -> c_long {
    unsafe { ffi::mi_option_get(option.raw()) }
}

/// Sets `option` to `value`, overriding any value from the environment.
pub fn option_set(option: MiOption, value: c_long) {
    unsafe { ffi::mi_option_set(option.raw(), value) }
}

/// Sets the default of `option`; a value from the environment still takes precedence.
///
/// Has no effect if the option was set explicitly or through the environment.
pub fn option_set_default(option: MiOption, value: c_long) {
    unsafe { ffi::mi_option_set_default(option.raw(), value) }
}

/// Returns whether the boolean `option` is enabled.
#[must_use]
pub fn option_is_enabled(option: MiOption) -> bool {
    unsafe { ffi::mi_option_is_enabled(option.raw()) }
}

/// Enables or disables the boolean `option`, overriding any value from the environment.
pub fn option_set_enabled(option: MiOption, enable: bool) {
    unsafe { ffi::mi_option_set_enabled(option.raw(), enable) }
}

/// Sets the default of the boolean `option`; a value from the environment still takes
/// precedence.
pub fn option_set_enabled_default(option: MiOption, enable: bool) {
    unsafe { ffi::mi_option_set_enabled_default(option.raw(), enable) }
}

/// Frees unused memory of the calling thread's heap and purges pending memory.
///
/// With `force`, all pending purges are executed regardless of their delay.
pub fn collect(force: bool) {
    unsafe { ffi::mi_collect(force) }
}

#[cfg(test)]
mod tests {
    use super::*;

    // (not `PurgeDelay`, whose feature default is checked concurrently in `lib.rs`)
    #[test]
    fn option_set_and_get() {
        let previous = option_get(MiOption::MaxWarnings);
        option_set(MiOption::MaxWarnings, 42);
        assert_eq!(option_get(MiOption::MaxWarnings), 42);
        option_set(MiOption::MaxWarnings, previous);
    }

    #[test]
    fn option_set_enabled_and_get() {
        let previous = option_is_enabled(MiOption::ShowStats);
        option_set_enabled(MiOption::ShowStats, true);
        assert!(option_is_enabled(MiOption::ShowStats));
        option_set_enabled(MiOption::ShowStats, previous);
    }

    #[test]
    fn collect_does_not_crash() {
        collect(false);
        collect(true);
    }
}
