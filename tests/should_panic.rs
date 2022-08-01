#![no_std]
#![no_main]

use os::{exit_qemu, serial_print, serial_println};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    should_fail();
    serial_println!("[test did not panic]");
    exit_qemu(os::QemuExitCode::Failed);

    loop {}
}

fn should_fail() {
    serial_print!("should_panic::should_fail... ");
    assert_eq!(0, 1);
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(os::QemuExitCode::Success);
    loop {}
}
