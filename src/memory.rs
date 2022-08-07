use bootloader::boot_info::{MemoryRegionKind, MemoryRegions};
use spin::{Mutex, MutexGuard, Once};
use x86_64::{
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, OffsetPageTable, PageTable, PageTableFlags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

use crate::{get_physical_memory_offset, serial_println, KERNEL_INFO};

static mut FRAME_ALLOCATOR: Once<Mutex<BootInfoFrameAllocator>> = Once::new();
static KERNEL_MAPPER: Once<Mutex<OffsetPageTable<'static>>> = Once::new();

pub fn lock_frame_allocator<'a>() -> MutexGuard<'a, BootInfoFrameAllocator> {
    unsafe { FRAME_ALLOCATOR.get().unwrap().lock() }
}

pub fn lock_memory_mapper<'a>() -> MutexGuard<'a, OffsetPageTable<'static>> {
    KERNEL_MAPPER.get().unwrap().lock()
}

pub fn init_memory() {
    let kernel_info = unsafe { KERNEL_INFO.get().unwrap() };
    let frame_allocator = unsafe { BootInfoFrameAllocator::new(&kernel_info.memory_regions) };
    unsafe {
        FRAME_ALLOCATOR.call_once(|| Mutex::new(frame_allocator));
    }
    let mapper = unsafe { init() };
    KERNEL_MAPPER.call_once(|| Mutex::new(mapper));
}

pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryRegions,
    next: usize,
}

impl BootInfoFrameAllocator {
    pub unsafe fn new(memory_map: &'static MemoryRegions) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        self.memory_map
            .iter()
            .filter(|r| r.kind == MemoryRegionKind::Usable)
            .map(|r| r.start..r.end)
            .flat_map(|r| r.step_by(4096))
            .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

pub fn allocate_page_table(
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> (OffsetPageTable<'static>, PhysFrame) {
    let physical_memory_offset = get_physical_memory_offset();

    let l4_frame = frame_allocator.allocate_frame().unwrap();
    let l3_frame = frame_allocator.allocate_frame().unwrap();
    let l2_frame = frame_allocator.allocate_frame().unwrap();

    let page_table_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
    let user_page_table_flags = page_table_flags | PageTableFlags::USER_ACCESSIBLE;

    let l4_address = l4_frame.start_address();
    let l3_address = l3_frame.start_address();
    let l2_address = l2_frame.start_address();

    let l4_table = (l4_address.as_u64() + physical_memory_offset) as *mut PageTable;
    let l4_table = unsafe { l4_table.as_mut().unwrap() };
    let l3_table = (l4_address.as_u64() + physical_memory_offset) as *mut PageTable;
    let l3_table = unsafe { l3_table.as_mut().unwrap() };
    let l2_table = (l4_address.as_u64() + physical_memory_offset) as *mut PageTable;
    let l2_table = unsafe { l2_table.as_mut().unwrap() };

    l4_table[0].set_addr(l3_address, user_page_table_flags);
    l3_table[0].set_addr(l2_address, user_page_table_flags);

    l2_table.iter_mut().take(16).for_each(|entry| {
        let frame: PhysFrame<Size4KiB> = frame_allocator.allocate_frame().unwrap();
        entry.set_addr(frame.start_address(), user_page_table_flags);
    });
    //TODO remove this

    unsafe {
        let mut mapper = init();
        let kernel_l4_table = mapper.level_4_table();
        serial_println!("L4 Page table:");
        for (i, entry) in kernel_l4_table.iter().enumerate() {
            if !entry.is_unused() {
                serial_println!("\t{:?} => {:?}", i, entry.addr());
                l4_table[i] = entry.clone();
            }
        }
        serial_println!("L3 Page table:");
        let kernel_l3_table = ((kernel_l4_table[0].addr().as_u64() + physical_memory_offset)
            as *const PageTable)
            .as_ref()
            .unwrap();
        for (i, entry) in kernel_l3_table.iter().enumerate() {
            if !entry.is_unused() {
                serial_println!("\t{:?} => {:?}", i, entry.addr());
            }
        }
    }

    let l4_frame = Cr3::read().0;
    let l4_table = (l4_frame.start_address().as_u64() + physical_memory_offset) as *mut PageTable;
    let l4_table = unsafe { l4_table.as_mut().unwrap() };
    unsafe {
        (
            OffsetPageTable::new(&mut *l4_table, VirtAddr::new(physical_memory_offset)),
            l4_frame,
        )
    }
}

pub unsafe fn init() -> OffsetPageTable<'static> {
    OffsetPageTable::new(
        active_level_4_table(),
        VirtAddr::new(get_physical_memory_offset()),
    )
}

unsafe fn active_level_4_table() -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = VirtAddr::new(get_physical_memory_offset()) + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}
