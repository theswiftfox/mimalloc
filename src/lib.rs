#![no_std]
#![doc = include_str!("../Readme.md")]

pub mod options;

mod ffi {
    use core::ffi::{c_int, c_void};

    // As `MI_SMALL_SIZE_MAX` in `mimalloc.h` (128 machine words)
    pub(crate) const MI_SMALL_SIZE_MAX: usize = 128 * core::mem::size_of::<usize>();
    // As `MI_MAX_ALIGN_SIZE`: `mi_malloc` returns blocks aligned to at least this
    pub(crate) const MI_MAX_ALIGN_SIZE: usize = 16;

    unsafe extern "C" {
        pub(crate) fn mi_malloc(size: usize) -> *mut c_void;
        pub(crate) fn mi_zalloc(size: usize) -> *mut c_void;
        pub(crate) fn mi_realloc(p: *mut c_void, newsize: usize) -> *mut c_void;
        pub(crate) fn mi_malloc_aligned(size: usize, alignment: usize) -> *mut c_void;
        pub(crate) fn mi_zalloc_aligned(size: usize, alignment: usize) -> *mut c_void;
        pub(crate) fn mi_realloc_aligned(
            p: *mut c_void,
            newsize: usize,
            alignment: usize,
        ) -> *mut c_void;
        pub(crate) fn mi_free(p: *mut c_void);
        pub(crate) fn mi_free_small_nonnull(p: *mut c_void);
        pub(crate) fn mi_usable_size(p: *const c_void) -> usize;
        pub(crate) fn mi_version() -> c_int;
        pub(crate) fn mi_stats_print_out(
            out: Option<unsafe extern "C" fn(*const core::ffi::c_char, *mut c_void)>,
            arg: *mut c_void,
        );
    }
}

use core::alloc::{GlobalAlloc, Layout};
use ffi::{MI_MAX_ALIGN_SIZE, MI_SMALL_SIZE_MAX};

/// The mimalloc allocator; use it as the `#[global_allocator]`.
#[derive(Clone, Copy, Debug, Default)]
pub struct MiMalloc;

/// The size for which `mi_malloc` returns an `align`-aligned block,
/// or `None` if `mi_malloc_aligned` is needed.
#[inline]
const fn malloc_size(size: usize, align: usize) -> Option<usize> {
    if align > MI_MAX_ALIGN_SIZE {
        None
    } else if size < align {
        Some(align)
    } else {
        Some(size)
    }
}

// Note: plain `mi_malloc` (which has its own small-size fast path) measured ~9% faster than
// upstream's `mi_wmalloc_small` on string-heavy workloads, and equal otherwise.
unsafe impl GlobalAlloc for MiMalloc {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe {
            match malloc_size(layout.size(), layout.align()) {
                Some(size) => ffi::mi_malloc(size),
                None => ffi::mi_malloc_aligned(layout.size(), layout.align()),
            }
        }
        .cast()
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe {
            match malloc_size(layout.size(), layout.align()) {
                Some(size) => ffi::mi_zalloc(size),
                None => ffi::mi_zalloc_aligned(layout.size(), layout.align()),
            }
        }
        .cast()
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Blocks of at most `MI_SMALL_SIZE_MAX` always live in small pages (also after an in-place
        // `realloc` shrink, which keeps at most 12.5% waste), so we can skip the page map lookup.
        unsafe {
            if layout.align() <= layout.size() && layout.size() <= MI_SMALL_SIZE_MAX {
                ffi::mi_free_small_nonnull(ptr.cast());
            } else {
                ffi::mi_free(ptr.cast());
            }
        }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe {
            match malloc_size(new_size, layout.align()) {
                Some(size) => ffi::mi_realloc(ptr.cast(), size),
                None => ffi::mi_realloc_aligned(ptr.cast(), new_size, layout.align()),
            }
        }
        .cast()
    }
}

impl MiMalloc {
    /// The usable size of an allocation (at least the requested size).
    ///
    /// # Safety
    /// `ptr` must be null or point to a live allocation of this allocator.
    #[inline]
    #[must_use]
    pub unsafe fn usable_size(&self, ptr: *const u8) -> usize {
        unsafe { ffi::mi_usable_size(ptr.cast()) }
    }
}

/// The mimalloc version, e.g. `30701` for v3.7.1.
#[must_use]
pub fn version() -> u32 {
    unsafe { ffi::mi_version().unsigned_abs() }
}

/// Print the allocator statistics to `stderr`.
pub fn stats_print() {
    unsafe { ffi::mi_stats_print_out(None, core::ptr::null_mut()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_frees_allocated_memory() {
        unsafe {
            let layout = Layout::from_size_align(8, 8).unwrap();
            let alloc = MiMalloc;

            let ptr = alloc.alloc(layout);
            alloc.dealloc(ptr, layout);
        }
    }

    #[test]
    fn it_frees_allocated_big_memory() {
        unsafe {
            let layout = Layout::from_size_align(1 << 20, 32).unwrap();
            let alloc = MiMalloc;

            let ptr = alloc.alloc(layout);
            alloc.dealloc(ptr, layout);
        }
    }

    #[test]
    fn it_frees_zero_allocated_memory() {
        unsafe {
            let layout = Layout::from_size_align(8, 8).unwrap();
            let alloc = MiMalloc;

            let ptr = alloc.alloc_zeroed(layout);
            alloc.dealloc(ptr, layout);
        }
    }

    #[test]
    fn it_frees_zero_allocated_big_memory() {
        unsafe {
            let layout = Layout::from_size_align(1 << 20, 32).unwrap();
            let alloc = MiMalloc;

            let ptr = alloc.alloc_zeroed(layout);
            alloc.dealloc(ptr, layout);
        }
    }

    #[test]
    fn it_frees_reallocated_memory() {
        unsafe {
            let layout = Layout::from_size_align(8, 8).unwrap();
            let alloc = MiMalloc;

            let ptr = alloc.alloc(layout);
            let ptr = alloc.realloc(ptr, layout, 16);
            alloc.dealloc(ptr, layout);
        }
    }

    #[test]
    fn it_frees_reallocated_big_memory() {
        unsafe {
            let layout = Layout::from_size_align(1 << 20, 32).unwrap();
            let alloc = MiMalloc;

            let ptr = alloc.alloc(layout);
            let ptr = alloc.realloc(ptr, layout, 2 << 20);
            alloc.dealloc(ptr, layout);
        }
    }

    #[test]
    fn it_respects_alignment_and_zeroing() {
        let alloc = MiMalloc;
        for align in [1, 2, 4, 8, 16, 32, 64, 128, 4096, 1 << 16] {
            for size in [
                1,
                7,
                8,
                15,
                16,
                24,
                100,
                1023,
                1024,
                1025,
                4000,
                70_000,
                1 << 20,
            ] {
                let layout = Layout::from_size_align(size, align).unwrap();
                unsafe {
                    let p = alloc.alloc(layout);
                    assert!(!p.is_null());
                    assert_eq!(p as usize % align, 0, "size {size} align {align}");
                    assert!(alloc.usable_size(p) >= size);
                    p.write_bytes(0xAB, size);
                    alloc.dealloc(p, layout);

                    let z = alloc.alloc_zeroed(layout);
                    assert_eq!(z as usize % align, 0);
                    assert!((0..size).all(|i| *z.add(i) == 0));
                    z.write_bytes(0xCD, size);
                    alloc.dealloc(z, layout);
                }
            }
        }
    }

    #[test]
    fn it_reallocs_across_size_classes() {
        let alloc = MiMalloc;
        for align in [1, 8, 16, 32, 256] {
            let sizes = [8, 900, 1024, 1100, 2000, 64, 20_000, 1000, 1 << 21, 512, 16];
            let mut layout = Layout::from_size_align(sizes[0], align).unwrap();
            unsafe {
                let mut p = alloc.alloc(layout);
                p.write_bytes(0x5A, layout.size());
                for &new_size in &sizes[1..] {
                    let keep = layout.size().min(new_size);
                    p = alloc.realloc(p, layout, new_size);
                    assert!(!p.is_null());
                    assert_eq!(p as usize % align, 0);
                    assert!((0..keep).all(|i| *p.add(i) == 0x5A));
                    layout = Layout::from_size_align(new_size, align).unwrap();
                    p.write_bytes(0x5A, new_size);
                }
                alloc.dealloc(p, layout);
            }
        }
    }

    #[test]
    fn it_has_a_version() {
        assert!(version() >= 30701);
    }

    // the features set the option defaults (unless overridden by `MIMALLOC_*` environment variables)
    #[test]
    fn it_applies_feature_option_defaults() {
        use crate::options::{MiOption, option_get};
        extern crate std;
        if std::env::vars().any(|(k, _)| k.starts_with("MIMALLOC_")) {
            return;
        }
        let purge_delay = if cfg!(feature = "purge_eager") {
            100
        } else if cfg!(feature = "purge_lazy") {
            10_000
        } else {
            1000
        };
        let arena_purge_mult = if cfg!(feature = "purge_eager") { 1 } else { 4 };
        assert_eq!(option_get(MiOption::PurgeDelay), purge_delay);
        assert_eq!(option_get(MiOption::ArenaPurgeMult), arena_purge_mult);
        if cfg!(target_os = "linux") {
            let thp = if cfg!(feature = "no_thp") {
                0
            } else if cfg!(feature = "thp_full") {
                2
            } else {
                1
            };
            assert_eq!(option_get(MiOption::AllowThp), thp);
        }
    }
}
