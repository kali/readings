use std::sync::atomic::AtomicUsize;

pub static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
pub static FREEED: AtomicUsize = AtomicUsize::new(0);

/// Setup global allocator instrumentation, to track rust-managed memory.
///
/// It is not mandatory to do so, as we also register the RSZ and VSZ as
/// reported by the OS, but it may be interesting. From our experience it may be
/// worth activating it as the cost is relatively modest.
///
/// This macro allows to specify a specific allocator instance to wrap and make global
/// (for instance, jemalloc).
#[macro_export]
macro_rules! wrap_global_allocator {
    ($alloc:path) => {
        #[global_allocator]
        static A: InstrumentedAllocator = InstrumentedAllocator;

        struct InstrumentedAllocator;

        unsafe impl std::alloc::GlobalAlloc for InstrumentedAllocator {
            unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
                use std::sync::atomic::Ordering::Relaxed;
                let ptr = $alloc.alloc(layout);
                if !ptr.is_null() {
                    $crate::alloc::ALLOCATED.fetch_add(layout.size(), Relaxed);
                }
                ptr
            }
            unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
                use std::sync::atomic::Ordering::Relaxed;
                if !ptr.is_null() {
                    $crate::alloc::FREEED.fetch_add(layout.size(), Relaxed);
                }
                $alloc.dealloc(ptr, layout);
            }
            unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, new_size: usize) -> *mut u8 {
                use std::sync::atomic::Ordering::*;
                if !ptr.is_null() {
                    $crate::alloc::FREEED.fetch_add(layout.size(), Relaxed);
                }
                let ptr = $alloc.realloc(ptr, layout, new_size);
                if !ptr.is_null() {
                    $crate::alloc::ALLOCATED.fetch_add(layout.size(), Relaxed);
                }
                ptr
            }
        }
    };
}

/// Setup global allocator instrumentation, to track rust-managed memory.
///
/// It is not mandatory to do so, as we also register the RSZ and VSZ as
/// reported by the OS, but it may be interesting. From our experience it may be
/// worth activating it as the cost is relatively modest.
///
/// This macro wrap the System allocator. Use `wrap_global_allocator` to wrap
/// another global allocator.
#[macro_export]
macro_rules! instrumented_allocator {
    () => {
        $crate::wrap_global_allocator!(std::alloc::System);
    };
}
