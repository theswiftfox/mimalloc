//! Runtime options and maintenance functions of mimalloc.
//!
//! Options can also be set through `MIMALLOC_<OPTION>` environment variables, see the
//! [mimalloc documentation](https://microsoft.github.io/mimalloc/environment.html).
//! Some options (e.g. [`MiOption::ArenaEagerCommit`]) are only read when the first arena
//! is created, so they only take effect when set before the first allocation.

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
/// `src/option_check.c`. Deprecated options are omitted.
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
    /// Eagerly commit segments (after `EagerCommitDelay` segments).
    EagerCommit = 3,
    /// Eagerly commit arenas; 2 enables it only on overcommit systems.
    ArenaEagerCommit = 4,
    /// Decommit memory on purge (1) or use memory reset instead (0).
    PurgeDecommits = 5,
    /// Allow large (2 or 4 MiB) OS pages.
    AllowLargeOsPages = 6,
    /// Reserve N huge (1 GiB) OS pages at startup.
    ReserveHugeOsPages = 7,
    /// Reserve huge OS pages at a specific NUMA node.
    ReserveHugeOsPagesAt = 8,
    /// Reserve the given amount of OS memory (in KiB) in an arena at startup.
    ReserveOsMemory = 9,
    /// Immediately purge delayed purges on thread termination.
    AbandonedPagePurge = 12,
    /// The first N segments per thread are not eagerly committed.
    EagerCommitDelay = 14,
    /// Delay purging of unused memory by N milliseconds; 0 purges immediately, -1 never.
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
    /// Max. percentage of the abandoned segments that can be reclaimed per try.
    MaxSegmentReclaim = 21,
    /// Release all memory on exit.
    DestroyOnExit = 22,
    /// Initial memory size (in KiB) for arena reservation.
    ArenaReserve = 23,
    /// Multiplier of `PurgeDelay` for purging arenas.
    ArenaPurgeMult = 24,
    /// Extend the purge delay on each subsequent delayed purge request.
    PurgeExtendDelay = 25,
    /// Do not use arenas for allocation (except with specific arena ids).
    DisallowArenaAlloc = 26,
    /// Retry on out-of-memory for N milliseconds (Windows only).
    RetryOnOom = 27,
    /// Allow visiting heap blocks from abandoned threads.
    VisitAbandoned = 28,
    /// Minimal rounded object size for guarded objects (requires `MI_GUARDED`).
    GuardedMin = 29,
    /// Maximal rounded object size for guarded objects (requires `MI_GUARDED`).
    GuardedMax = 30,
    /// Always place guarded blocks exactly in front of a guard page.
    GuardedPrecise = 31,
    /// Guard 1 out of N allocations in the min/max range.
    GuardedSampleRate = 32,
    /// Seed for the guarded sampling.
    GuardedSampleSeed = 33,
    /// Experimental: target number of segments per thread.
    TargetSegmentsPerThread = 34,
    /// Allow reclaiming an abandoned segment on a free.
    ReclaimOnFree = 35,
    /// Retain N full pages per size class.
    PageFullRetain = 36,
    /// Max. number of candidate pages to consider for allocation.
    PageMaxCandidates = 37,
    /// Max. user space virtual address bits to consider.
    MaxVabits = 38,
    /// Commit the full pagemap.
    PagemapCommit = 39,
    /// Commit page memory on demand.
    PageCommitOnDemand = 40,
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
/// Has no effect if the option was already read or set.
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

    #[test]
    fn option_set_and_get() {
        let previous = option_get(MiOption::PurgeDelay);
        option_set(MiOption::PurgeDelay, 42);
        assert_eq!(option_get(MiOption::PurgeDelay), 42);
        option_set(MiOption::PurgeDelay, previous);
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
