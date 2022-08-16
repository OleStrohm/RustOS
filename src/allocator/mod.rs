pub mod bump;
pub mod fixed_size_block;
pub mod linked_list;

use fixed_size_block::FixedSizeBlockAllocator;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

use crate::{memory::{lock_frame_allocator, lock_memory_mapper, print_page_table}, serial_println};

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(FixedSizeBlockAllocator::new());

pub const HEAP_START: usize = 0xFFFF_A000_0000_0000;
pub const HEAP_SIZE: usize = 100 * 1024;

pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }
}

impl<A> core::ops::Deref for Locked<A> {
    type Target = spin::Mutex<A>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub fn init_heap() -> Result<(), MapToError<Size4KiB>> {
    let virt = VirtAddr::new(HEAP_START as u64);
    serial_println!("Heap indices: {:?}, {:?}, {:?}, {:?}", virt.p4_index(), virt.p3_index(), virt.p2_index(), virt.p1_index());
    let mut frame_allocator = lock_frame_allocator();
    let mut mapper = lock_memory_mapper();
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe { mapper.map_to(page, frame, flags, &mut *frame_allocator)?.flush() }
    }

    print_page_table(&mut mapper);

    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}

fn align_up(addr: usize, align: usize) -> usize {
    let remainder = addr % align;
    if remainder == 0 {
        addr
    } else {
        addr - remainder + align
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;

    #[test_case]
    fn simple_box() {
        let b = Box::new(5);
        assert_eq!(*b, 5);
    }
}
