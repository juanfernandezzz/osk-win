// Diagnostic allocator only. Counts Rust System allocation requests on this thread;
// excludes C/C++, Win32/GDI and other threads. Never use these runs as timing results.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static COUNTS: Cell<(usize, usize, usize)> = const { Cell::new((0, 0, 0)) };
}

fn count(bytes: usize, realloc: bool) {
    let _ = ACTIVE.try_with(|active| if active.get() {
        COUNTS.with(|counts| {
            let (n, b, r) = counts.get();
            counts.set((n + 1, b + bytes, r + usize::from(realloc)));
        });
    });
}

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size(), false);
        System.alloc(layout)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size(), false);
        System.alloc_zeroed(layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count(size, true);
        System.realloc(ptr, layout, size)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) { System.dealloc(ptr, layout); }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

pub fn measure(f: impl FnOnce()) -> (usize, usize, usize) {
    struct Reset;
    impl Drop for Reset { fn drop(&mut self) { ACTIVE.with(|a| a.set(false)); } }
    COUNTS.with(|c| c.set((0, 0, 0)));
    ACTIVE.with(|a| assert!(!a.replace(true)));
    let reset = Reset;
    f();
    drop(reset);
    COUNTS.with(Cell::get)
}
