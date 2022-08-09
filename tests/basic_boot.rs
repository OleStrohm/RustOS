#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(os::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]

use bootloader::{entry_point, BootInfo};
use os::println;

#[test_case]
fn test_println() {
    println!("test_println output");
}

entry_point!(main);

fn main(boot_info: &'static mut BootInfo) -> ! {
    os::init(boot_info);

    test_main();
    os::hlt_loop();
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    os::tests::test_panic_handler(info);
}
