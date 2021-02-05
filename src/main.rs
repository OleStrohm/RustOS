#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(os::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use alloc::{boxed::Box, vec, vec::Vec, rc::Rc};

use bootloader::{entry_point, BootInfo};
use memory::BootInfoFrameAllocator;
use os::{memory, println};
use x86_64::VirtAddr;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    println!("Hello world!");
    os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::new(&boot_info.memory_map) };

    os::allocator::init_heap(&mut mapper, &mut frame_allocator).expect("Heap initalization failed");

    {
        let heap_value = Box::new(41);
        println!("Heap value: {}", heap_value);

        let mut vec = Vec::new();
        for i in 0..5 {
            vec.push(i);
        }
        println!("Vec: {:?}", vec);

        let reference_counted = Rc::new(vec![1, 2, 3]);
        let cloned_reference = reference_counted.clone();
        println!("Current reference count is {}", Rc::strong_count(&cloned_reference));
        core::mem::drop(reference_counted);
        println!("Current reference count is {}", Rc::strong_count(&cloned_reference));
    }

    #[cfg(test)]
    test_main();
    println!("It didn't crash!");
    os::hlt_loop();
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
