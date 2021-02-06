#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(os::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use memory::BootInfoFrameAllocator;
use os::{memory, print, println, task::{executor::Executor, keyboard, Task}};
use pc_keyboard::DecodedKey;
use x86_64::VirtAddr;

entry_point!(kernel_main);

async fn print_keypresses() {
    loop {
        match os::task::keyboard::recv().await {
            DecodedKey::Unicode(c) => print!("{}", c),
            DecodedKey::RawKey(key) => print!("{:?}", key),
        }
    }
}

async fn async_number() -> u32 {
    42
}

async fn example_task() {
    let number = async_number().await;
    println!("async number: {}", number);
}

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    println!("Hello world!");
    os::init();
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::new(&boot_info.memory_map) };
    os::allocator::init_heap(&mut mapper, &mut frame_allocator).expect("Heap initalization failed");

    let mut executor = Executor::new();
    executor.spawn(Task::new(example_task()));
    executor.spawn(Task::new(keyboard::keyboard_scheduler()));
    executor.spawn(Task::new(print_keypresses()));
    executor.spawn(Task::new(print_keypresses()));
    executor.run();
}

#[cfg(test)]
mod tests {
    #[test_case]
    fn trivial_assertion() {
        assert_eq!(0, 0);
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{}", info);
    os::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    os::tests::test_panic_handler(info);
}
