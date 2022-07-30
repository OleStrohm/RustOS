use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::structures::idt::InterruptStackFrameValue;
use x86_64::structures::paging::{
    mapper, FrameAllocator, Mapper, Page, PageTableFlags as Flags, Size4KiB,
};
use x86_64::VirtAddr;

#[derive(Debug, Clone, Copy)]
pub struct Thread {
    pub tid: ThreadId,
    //pub regs: ThreadRegisters,
    pub stack_frame: Option<InterruptStackFrameValue>,
    pub regs: Option<Registers>,
}

impl Thread {
    pub fn create_entrypoint(
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        entrypoint: fn() -> !,
    ) -> Self {
        let stack = Stack::allocate(10, mapper, frame_allocator);
        // /*(stack_frame, regs) = */stack.setup_for_entry(entrypoint);

        Thread {
            tid: ThreadId::new(),
            stack_frame: Some(InterruptStackFrameValue {
                instruction_pointer: VirtAddr::new(entrypoint as u64),
                code_segment: 8,
                cpu_flags: 0x200,
                stack_pointer: stack.end,
                stack_segment: 0,
            }),
            regs: Some(Registers::default()),
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
    fn allocate(
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
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rdi: u64,
    rsi: u64,
    rbp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
}
