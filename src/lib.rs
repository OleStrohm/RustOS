#![no_std]
#![feature(never_type)]
#![feature(alloc_error_handler)]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![feature(abi_x86_interrupt)]
#![feature(const_mut_refs)]
#![feature(naked_functions)]
#![feature(asm_sym)]
#![feature(asm_const)]
#![test_runner(crate::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

pub mod allocator;
pub mod backtrace;
pub mod gdt;
pub mod interrupts;
pub mod memory;
pub mod serial;
pub mod task;
pub mod vga;

use self::memory::BootInfoFrameAllocator;
use bootloader::BootInfo;
use core::alloc::Layout;
use task::scheduler;
use x86_64::VirtAddr;

#[cfg(test)]
use bootloader::entry_point;

#[cfg(test)]
entry_point!(test_kernel_main);

#[cfg(test)]
fn test_kernel_main(bootinfo: &'static BootInfo) -> ! {
    init(bootinfo);
    test_main();
    hlt_loop();
}

pub fn init(boot_info: &'static BootInfo) {
    interrupts::init();
    gdt::init();
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::new(&boot_info.memory_map) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("Heap initalization failed");

    scheduler::init_scheduler(mapper, frame_allocator);
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

pub mod tests {
    use super::{exit_qemu, hlt_loop, QemuExitCode};
    use crate::{serial_print, serial_println};

    pub trait Testable {
        fn run(&self) -> ();
    }

    impl<T> Testable for T
    where
        T: Fn(),
    {
        fn run(&self) {
            serial_print!("{}... ", core::any::type_name::<T>());
            self();
            serial_println!("[OK]");
        }
    }

    pub fn test_runner(tests: &[&dyn Testable]) {
        serial_println!("Running {} tests", tests.len());
        for test in tests {
            test.run();
        }

        exit_qemu(QemuExitCode::Success);
    }

    pub fn test_panic_handler(info: &core::panic::PanicInfo) -> ! {
        serial_println!("[FAILED]\n");
        serial_println!("Error: {info}\n");
        exit_qemu(QemuExitCode::Failed);
        hlt_loop();
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("allocation error: {layout:?}")
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    tests::test_panic_handler(info);
}
