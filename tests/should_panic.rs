#![no_std]
#![no_main]

use os::{exit_qemu, serial_print, serial_println};
use owo_colors::OwoColorize;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    should_fail();
    serial_println!("{}", "[FAILED]".red());
    exit_qemu(os::QemuExitCode::Failed);

    loop {}
}

fn should_fail() {
    serial_print!("should_panic::should_fail... ");
    assert_eq!(0, 1);
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    serial_println!("{}", "[OK]".green());
    exit_qemu(os::QemuExitCode::Success);
    loop {}
}
