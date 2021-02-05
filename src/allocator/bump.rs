use super::{align_up, Locked};
use core::alloc::{GlobalAlloc, Layout};

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
    alloc_counter: u64,
}

impl BumpAllocator {
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            alloc_counter: 0,
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
    }
}

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut bump = self.lock();

        let alloc_start = align_up(bump.next, layout.align());
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(end) => end,
            None => return core::ptr::null_mut(),
        };

        if alloc_end > bump.heap_end {
            core::ptr::null_mut()
        } else {
            bump.next = alloc_start + layout.size();
            bump.alloc_counter += 1;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {
        let mut bump = self.lock();

        bump.alloc_counter -= 1;
        if bump.alloc_counter == 0 {
            bump.next = bump.heap_start;
        }
    }
}
