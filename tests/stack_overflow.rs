#![feature(abi_x86_interrupt)]
#![no_std]
#![no_main]

use lazy_static::lazy_static;
use os::{exit_qemu, serial_print, serial_println};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use owo_colors::OwoColorize;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial_print!("stack_overflow::stack_overflow... ");

    os::gdt::init();
    init_test_idt();

    stack_overflow();

    panic!("Execution continued after stack overflow");
}

#[allow(unconditional_recursion)]
fn stack_overflow() {
    stack_overflow();
    unsafe {
        core::ptr::read_volatile(0 as *const u32);
    }
}

lazy_static! {
    static ref TEST_IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        unsafe {
            idt.double_fault
                .set_handler_fn(test_double_fault_handler)
                .set_stack_index(os::gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt
    };
}

extern "x86-interrupt" fn test_double_fault_handler(_: InterruptStackFrame, _: u64) -> ! {
    serial_println!("{}", "[OK]".green());
    exit_qemu(os::QemuExitCode::Success);
    loop {}
}

pub fn init_test_idt() {
    TEST_IDT.load();
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    serial_println!("{}", "[FAILED]".red());
    exit_qemu(os::QemuExitCode::Failed);
    loop {}
}
