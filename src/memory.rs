use bootloader::boot_info::{MemoryRegionKind, MemoryRegions};
use spin::{Mutex, MutexGuard, Once};
use x86_64::{
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, OffsetPageTable, PageTable, PageTableFlags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

use crate::{get_memory_regions, get_physical_memory_offset, serial_println};

static FRAME_ALLOCATOR: Once<Mutex<BootInfoFrameAllocator>> = Once::new();
static KERNEL_MAPPER: Once<Mutex<OffsetPageTable<'static>>> = Once::new();

pub fn lock_frame_allocator<'a>() -> MutexGuard<'a, BootInfoFrameAllocator> {
    FRAME_ALLOCATOR.get().unwrap().lock()
}

pub fn lock_memory_mapper<'a>() -> MutexGuard<'a, OffsetPageTable<'static>> {
    KERNEL_MAPPER.get().unwrap().lock()
}

pub fn init_memory() {
    let frame_allocator = unsafe { BootInfoFrameAllocator::new(get_memory_regions()) };
    FRAME_ALLOCATOR.call_once(|| Mutex::new(frame_allocator));
    let mapper = unsafe { init() };
    KERNEL_MAPPER.call_once(|| Mutex::new(mapper));
}

pub fn print_page_table(mapper: &mut OffsetPageTable<'static>) {
    serial_println!("hi");
    let l4 = mapper.level_4_table();
    serial_println!("hi");
    for (i, entry) in l4.iter().enumerate() {
        if !entry.is_unused() {
            serial_println!("l4[{}] => {:?}", i, entry.addr());
        }
    }
    serial_println!("hi");
    let l3 = (get_physical_memory_offset() + l4[0].addr().as_u64()) as *mut PageTable;
    serial_println!("hi");
    let l3 = unsafe { &mut *l3 };
    serial_println!("hi");
    for (i, entry) in l3.iter().enumerate() {
        if !entry.is_unused() {
            serial_println!("l3[{}] => {:?}", i, entry.addr());
        }
    }
}

pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryRegions,
    next: usize,
}

unsafe impl Send for BootInfoFrameAllocator {}

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
    mapper: &mut OffsetPageTable<'static>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> (PhysFrame, PhysAddr) {
    let physical_memory_offset = get_physical_memory_offset();

    let l4_frame = frame_allocator.allocate_frame().unwrap();
    let l3_frame: PhysFrame<Size4KiB> = frame_allocator.allocate_frame().unwrap();
    let l2_frame: PhysFrame<Size4KiB> = frame_allocator.allocate_frame().unwrap();
    let l1_frame: PhysFrame<Size4KiB> = frame_allocator.allocate_frame().unwrap();

    let kernel_page_table_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
    let user_page_table_flags =
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;

    let l4_address = l4_frame.start_address();
    let l3_address = l3_frame.start_address();
    let l2_address = l2_frame.start_address();
    let l1_address = l1_frame.start_address();

    let l4_table = (l4_address.as_u64() + physical_memory_offset) as *mut PageTable;
    let l4_table = unsafe { l4_table.as_mut().unwrap() };
    let l3_table = (l3_address.as_u64() + physical_memory_offset) as *mut PageTable;
    let l3_table = unsafe { l3_table.as_mut().unwrap() };
    let l2_table = (l2_address.as_u64() + physical_memory_offset) as *mut PageTable;
    let l2_table = unsafe { l2_table.as_mut().unwrap() };
    let l1_table = (l1_address.as_u64() + physical_memory_offset) as *mut PageTable;
    let l1_table = unsafe { l1_table.as_mut().unwrap() };

    l4_table[0].set_addr(l3_address, user_page_table_flags);
    l3_table[0].set_addr(l2_address, user_page_table_flags);
    l2_table[0].set_addr(l1_address, user_page_table_flags);

    let (kernel_data, kernel_code) = get_kernel_level_3_tables(mapper);
    l3_table[510].set_addr(kernel_data, kernel_page_table_flags);
    l3_table[511].set_addr(kernel_code, kernel_page_table_flags);

    l1_table.iter_mut().take(16).for_each(|entry| {
        let frame = frame_allocator.allocate_frame().unwrap();
        entry.set_addr(frame.start_address(), user_page_table_flags);
    });

    (l4_frame, l1_table[0].addr())
}

fn get_kernel_level_3_tables(mapper: &mut OffsetPageTable<'static>) -> (PhysAddr, PhysAddr) {
    let kernel_l4_table = mapper.level_4_table();
    let kernel_l3_table = unsafe {
        ((kernel_l4_table[0].addr().as_u64() + mapper.phys_offset().as_u64()) as *const PageTable)
            .as_ref()
            .unwrap()
    };
    (kernel_l3_table[510].addr(), kernel_l3_table[511].addr())
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
