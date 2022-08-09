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
    use alloc::{boxed::Box, vec, vec::Vec};

    #[test_case]
    fn simple_allocation() {
        let heap_value_1 = Box::new(41);
        let heap_value_2 = Box::new(13);

        assert_eq!(*heap_value_1, 41);
        assert_eq!(*heap_value_2, 13);
    }

    #[test_case]
    fn small_vec() {
        let vec = vec![1, 2, 3, 4, 5];

        assert_eq!(vec.iter().sum::<u64>(), 15);
    }

    #[test_case]
    fn large_vec() {
        let n = 1000;
        let mut vec = Vec::new();
        for i in 0..n {
            vec.push(i);
        }

        assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
    }

    #[test_case]
    fn too_many_boxes() {
        use os::allocator::HEAP_SIZE;
        for i in 0..HEAP_SIZE {
            let x = Box::new(i);
            assert_eq!(*x, i);
        }
    }

    #[test_case]
    fn too_many_boxes_long_lived() {
        use os::allocator::HEAP_SIZE;
        let long_lived = Box::new(0);
        for i in 0..HEAP_SIZE {
            let x = Box::new(i);
            assert_eq!(*x, i);
        }
        assert_eq!(*long_lived, 0);
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    os::tests::test_panic_handler(info);
}
