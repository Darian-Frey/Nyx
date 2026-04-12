//! A custom global allocator that panics when heap allocation is attempted
//! inside a guarded scope (the audio callback).
//!
//! # How it works
//!
//! A thread-local flag marks whether the current thread is inside the
//! audio callback. The `DenyAllocGuard` RAII type sets and clears this flag.
//! The `GuardedAllocator` wraps `std::alloc::System` and checks the flag
//! on every `alloc` and `realloc`.
//!
//! `dealloc` is intentionally NOT guarded — the panic unwind machinery
//! must be able to free memory, and preventing drops in the audio callback
//! is the caller's responsibility (don't hold heap-allocated values that
//! get dropped per-sample).
//!
//! # Usage
//!
//! In tests, wrap the signal-under-test in a `DenyAllocGuard` scope:
//!
//! ```ignore
//! let _guard = DenyAllocGuard::new();
//! signal.next(&ctx); // panics if this allocates
//! ```
//!
//! In production, the engine sets the guard around the audio callback.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static DENY_ALLOC: Cell<bool> = const { Cell::new(false) };
}

/// Returns `true` if the current thread is inside a no-alloc guard.
fn is_alloc_denied() -> bool {
    DENY_ALLOC.with(|f| f.get())
}

/// RAII guard that denies heap allocation for the current thread while held.
pub struct DenyAllocGuard {
    _private: (),
}

impl Default for DenyAllocGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl DenyAllocGuard {
    /// Enter the no-alloc zone. Any heap allocation on this thread will
    /// panic until the guard is dropped.
    pub fn new() -> Self {
        DENY_ALLOC.with(|f| f.set(true));
        DenyAllocGuard { _private: () }
    }
}

impl Drop for DenyAllocGuard {
    fn drop(&mut self) {
        DENY_ALLOC.with(|f| f.set(false));
    }
}

/// A global allocator wrapper that panics when allocation is attempted
/// inside a `DenyAllocGuard` scope.
///
/// To activate, add this to your test binary or main:
///
/// ```ignore
/// #[global_allocator]
/// static ALLOC: nyx_core::GuardedAllocator = nyx_core::GuardedAllocator;
/// ```
pub struct GuardedAllocator;

unsafe impl GlobalAlloc for GuardedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if is_alloc_denied() {
            // Clear the flag BEFORE panicking — the panic machinery itself
            // needs to allocate (format strings, backtraces, etc.).
            DENY_ALLOC.with(|f| f.set(false));
            panic!(
                "nyx: heap allocation of {} bytes in no-alloc zone (audio callback)",
                layout.size()
            );
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Intentionally not guarded — panic unwind needs to free memory.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if is_alloc_denied() {
            DENY_ALLOC.with(|f| f.set(false));
            panic!(
                "nyx: heap reallocation from {} to {} bytes in no-alloc zone (audio callback)",
                layout.size(),
                new_size
            );
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}
