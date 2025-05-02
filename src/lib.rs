#![no_std]

#[allow(dead_code)]
mod ffi {
    use core::ffi::c_void;

    unsafe extern "C" {
        pub(crate) fn mi_zalloc(size: usize) -> *mut c_void;
        pub(crate) fn mi_malloc(size: usize) -> *mut c_void;
        pub(crate) fn mi_realloc(p: *mut c_void, newsize: usize) -> *mut c_void;
        pub(crate) fn mi_zalloc_aligned(size: usize, alignment: usize) -> *mut c_void;
        pub(crate) fn mi_malloc_aligned(size: usize, alignment: usize) -> *mut c_void;
        pub(crate) fn mi_realloc_aligned(
            p: *mut c_void,
            newsize: usize,
            alignment: usize,
        ) -> *mut c_void;
        pub(crate) fn mi_free(p: *mut c_void);
    }
}

use core::alloc::{GlobalAlloc, Layout};
use core::ffi::c_void;

pub struct MiMalloc;

unsafe impl GlobalAlloc for MiMalloc {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { ffi::mi_malloc_aligned(layout.size(), layout.align()).cast::<u8>() }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe { ffi::mi_zalloc_aligned(layout.size(), layout.align()).cast::<u8>() }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe { ffi::mi_free(ptr.cast::<c_void>()) }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe { ffi::mi_realloc_aligned(ptr.cast::<c_void>(), new_size, layout.align()).cast::<u8>() }
    }
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
}
