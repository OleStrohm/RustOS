#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(os::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::arch::asm;

use bootloader::{entry_point, BootInfo};
use os::{
    print, println, serial_println,
    task::{executor::Executor, keyboard, scheduler, Task},
};
use pc_keyboard::DecodedKey;

entry_point!(kernel_main);

async fn print_keypresses() {
    loop {
        match os::task::keyboard::recv().await {
            DecodedKey::Unicode(c) => {
                serial_println!("printing {}", c);
                print!("{}", c);
            }
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

fn user_task() -> ! {
    unsafe {
        asm!(
            "
                mov rax, 12
                int 0x80
                2:
                mov rax, 1
                jmp 2b
            ",
            options(noreturn)
        );
    }
}

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    os::init(boot_info);
    if cfg!(test) {
        #[cfg(test)]
        test_main();
        os::hlt_loop();
    }
    fn slow() {
        let mut sum: i32 = 0;
        for i in 0..400000 {
            sum = sum.wrapping_add(i);
        }
        //assert_eq!(sum, 32);
    }

    println!("Hey there");
    serial_println!("Hey there");

    //unsafe {
    //    let mut mapper = memory::init();
    //    let l4_table = mapper.level_4_table();
    //    serial_println!("L4 Page table:");
    //    for (i, entry) in l4_table.iter().enumerate() {
    //        if !entry.is_unused() {
    //            serial_println!("\t{:?} => {:?}", i, entry.addr());
    //        }
    //    }
    //}

    scheduler::spawn(|| loop {
        slow();
        print!("2");
    });
    //scheduler::spawn(|| loop {
    //    slow();
    //    print!("3");
    //});
    scheduler::spawn_user(user_task);
    //scheduler::spawn(user_task);
    //scheduler::spawn_user(|| loop {
    //    //slow();
    //    //print!("4");
    //    unsafe {
    //        asm!("
    //            mov rax, 1
    //            hlt
    //        ");
    //    }
    //});

    let mut executor = Executor::new();
    executor.spawn(Task::new(example_task()));
    executor.spawn(Task::new(keyboard::keyboard_scheduler()));
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

#[cfg(test)]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    os::tests::test_panic_handler(info);
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    serial_println!("{}", info);
    println!("{}", info);
    os::hlt_loop();
}
