use core::cell::UnsafeCell;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::instructions::tables::load_tss;
use x86_64::registers::segmentation::{Segment, CS};
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
        let current_tss = gdt.add_entry(Descriptor::tss_segment(unsafe { &*TSS.lock().get() }));
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
    ) = &*GDT;

    gdt.load();
    unsafe {
        CS::set_reg(*code_selector);
        load_tss(*current_tss);
    }
}
