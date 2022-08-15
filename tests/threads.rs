#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(os::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{entry_point, BootInfo};

entry_point!(main);

fn main(boot_info: &'static mut BootInfo) -> ! {
    os::init(boot_info);

    test_main();
    os::hlt_loop();
}

mod tests {
    use core::sync::atomic::{AtomicBool, Ordering};

    use os::task::scheduler;

    #[test_case]
    fn simple_kernel_thread() {
        static OTHER_THREAD: AtomicBool = AtomicBool::new(false);
        scheduler::spawn(|| {
            OTHER_THREAD.store(true, Ordering::SeqCst);
            os::hlt_loop();
        });
        while !OTHER_THREAD.load(Ordering::SeqCst) {}
    }

    #[test_case]
    fn simple_user() {
        scheduler::spawn_user(|| loop {});
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    os::tests::test_panic_handler(info);
}
