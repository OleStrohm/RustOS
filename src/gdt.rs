use crate::task::thread::{Stack, ThreadId};
use core::cell::UnsafeCell;
use lazy_static::{__Deref, lazy_static};
use spin::Mutex;
use x86_64::instructions::segmentation::set_cs;
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    static ref TSS: Mutex<UnsafeCell<TaskStateSegment>> = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            let stack_end = stack_start + STACK_SIZE;
            stack_end
        };
        Mutex::new(UnsafeCell::new(tss))
    };
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let current_tss = unsafe { gdt.add_entry(Descriptor::tss_segment(&*TSS.lock().get())) };
        (
            gdt,
            Selectors {
                code_selector,
                current_tss,
            },
        )
    };
}

struct Selectors {
    code_selector: SegmentSelector,
    current_tss: SegmentSelector,
}

pub fn init() {
    let (
        gdt,
        Selectors {
            code_selector,
            current_tss,
        },
    ) = GDT.deref();

    gdt.load();
    unsafe {
        set_cs(*code_selector);
        load_tss(*current_tss);
    }
}

pub unsafe fn get_int_stack_addr() -> VirtAddr {
    (*TSS.lock().get()).interrupt_stack_table[0]
}

pub unsafe fn context_switch_to(tid: ThreadId, stack: Stack) {
    let mut tss = TSS.lock();
    tss.get_mut().privilege_stack_table[0] = stack.rsp;

    init();
}
