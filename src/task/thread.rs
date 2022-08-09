use core::ptr::copy_nonoverlapping;
use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::registers::control::Cr3;
use x86_64::structures::idt::InterruptStackFrameValue;
use x86_64::structures::paging::{
    mapper, FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags as Flags,
    PhysFrame, Size4KiB,
};
use x86_64::VirtAddr;

use crate::gdt::GDT;
use crate::{get_physical_memory_offset, memory, println, serial_println};

#[derive(Debug, Clone, Copy)]
pub struct Thread {
    pub tid: ThreadId,
    pub stack_frame: Option<InterruptStackFrameValue>,
    pub regs: Option<Registers>,
}

impl Thread {
    pub fn create_userspace_entrypoint(
        mapper: &mut OffsetPageTable<'static>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        entrypoint: fn() -> !,
    ) -> Self {
        let (cr3, user_zero) = memory::allocate_page_table(mapper, frame_allocator);

        unsafe {
            let phys = cr3.start_address();
            let virt = VirtAddr::new(get_physical_memory_offset()) + phys.as_u64();
            let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
            let user_mapper = OffsetPageTable::new(&mut *page_table_ptr, mapper.phys_offset());
            let translated_user_zero = user_mapper
                .translate_page(Page::<Size4KiB>::containing_address(VirtAddr::new(0x9ff8)));
            serial_println!("0x9ff8 => {translated_user_zero:?}");
        }
        let stack = VirtAddr::new(2 * 4096); // at least 10 pages have been allocated
        let entrypoint = unsafe {
            let start_of_code = 4096; // One page in
            copy_nonoverlapping(
                entrypoint as *const u8,
                (user_zero.as_u64() + start_of_code + get_physical_memory_offset()) as *mut u8,
                4096,
            );
            for i in 0..5 {
                let target =
                    (user_zero.as_u64() + start_of_code + get_physical_memory_offset()) as *mut u8;
                let target = target.add(i);
                serial_println!("0x1000+{:}: {:X}", i, target.read());
            }
            VirtAddr::new(start_of_code)
        };

        //TODO: REMOVE
        //let stack = Stack::allocate_kernel(10, mapper, frame_allocator).end;
        //let entrypoint = VirtAddr::new(entrypoint as u64);
        //let (cr3, _) = Cr3::read();

        Thread {
            tid: ThreadId::new(),
            stack_frame: Some(InterruptStackFrameValue {
                instruction_pointer: entrypoint,
                code_segment: GDT.1.user_code_selector.0 as u64, //GDT.1.kernel_code_selector.0 as u64, //0x18 | 3, // Selector index 0x18 (3 * 8) and ring 3 (lower two bits)
                cpu_flags: 0x200,
                stack_pointer: stack,
                stack_segment: GDT.1.user_data_selector.0 as u64,
            }),
            regs: Some(Registers::with_cr3(cr3)),
        }
    }

    pub fn create_entrypoint(
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        entrypoint: fn() -> !,
    ) -> Self {
        let stack = Stack::allocate_kernel(10, mapper, frame_allocator);

        println!(
            "User code segment(idx:{}): {:?}",
            GDT.1.user_code_selector.0 as u64, GDT.1.user_code_selector
        );
        println!(
            "Kernel code segment(idx:{}): {:?}",
            GDT.1.kernel_code_selector.0 as u64, GDT.1.kernel_code_selector
        );
        let (cr3, _) = Cr3::read();
        Thread {
            tid: ThreadId::new(),
            stack_frame: Some(InterruptStackFrameValue {
                instruction_pointer: VirtAddr::new(entrypoint as u64),
                code_segment: GDT.1.kernel_code_selector.0 as u64,
                cpu_flags: 0x202,
                stack_pointer: stack.end,
                stack_segment: GDT.1.kernel_data_selector.0 as u64,
            }),
            regs: Some(Registers::with_cr3(cr3)),
        }
    }

    pub fn create_root_thread() -> Thread {
        Thread {
            tid: ThreadId::initial(),
            stack_frame: None,
            regs: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Stack {
    pub end: VirtAddr,
}

impl Stack {
    fn allocate_kernel(
        size_in_pages: u64,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Self {
        Self::alloc_stack(size_in_pages, mapper, frame_allocator).unwrap()
    }

    fn alloc_stack(
        size_in_pages: u64,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Result<Stack, mapper::MapToError<Size4KiB>> {
        static STACK_ALLOC_NEXT: AtomicU64 = AtomicU64::new(0x_5555_5555_0000);

        let guard_page_start = STACK_ALLOC_NEXT.fetch_add(
            (size_in_pages + 1) * Page::<Size4KiB>::SIZE,
            Ordering::SeqCst,
        );
        let guard_page = Page::from_start_address(VirtAddr::new(guard_page_start))
            .expect("`STACK_ALLOC_NEXT` not page aligned");

        let stack_start = guard_page + 1;
        let stack_end = stack_start + size_in_pages;
        let flags = Flags::PRESENT | Flags::WRITABLE;
        for page in Page::range(stack_start, stack_end) {
            let frame = frame_allocator
                .allocate_frame()
                .ok_or(mapper::MapToError::FrameAllocationFailed)?;
            unsafe {
                mapper.map_to(page, frame, flags, frame_allocator)?.flush();
            }
        }
        Ok(Stack {
            end: stack_end.start_address(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct ThreadId(u64);

impl ThreadId {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        ThreadId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub fn initial() -> Self {
        ThreadId(0)
    }

    pub unsafe fn from_u64(tid: u64) -> Self {
        ThreadId(tid)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl Into<u64> for ThreadId {
    fn into(self) -> u64 {
        let ThreadId(inner) = self;
        inner
    }
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct Registers {
    pub cr3: u64,
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

impl Registers {
    fn with_cr3(cr3: PhysFrame) -> Self {
        Self {
            cr3: cr3.start_address().as_u64(),
            ..Default::default()
        }
    }
}
