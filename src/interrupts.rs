use crate::task::scheduler::{self, add_paused_thread, current_thread};
use crate::task::thread::Registers;
use crate::{gdt, hlt_loop, println, serial_println};
use core::arch::asm;
use core::mem::size_of;
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use x86_64::VirtAddr;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

macro_rules! push_registers {
    () => {
        "push r15; push r14; push r13; push r12; push r11; push r10; push r9; push r8; push rbp;
         push rsi; push rdi; push rdx; push rcx; push rbx; push rax; mov rax, cr3; push rax"
    };
}

macro_rules! pop_registers {
    () => {
        "pop rax; mov cr3, rax; pop rax; pop rbx; pop rcx; pop rdx; pop rdi; pop rsi;
         pop rbp; pop r8; pop r9; pop r10; pop r11; pop r12; pop r13; pop r14; pop r15"
    };
}

macro_rules! register_interrupt {
    ($idt:ident, $interrupt:path => $handler:ident) => {{
        #[allow(unused)]
        const CHECK_HANDLER: extern "C" fn(stack_frame: &mut InterruptStackFrame, regs: &mut Registers) = $handler;
        #[naked]
        extern "x86-interrupt" fn handler() {
            unsafe {
                asm!(
                    push_registers!(),
                    "
                    mov rdi, rsp
                    add rdi, {regs_size}
                    mov rsi, rsp
                    sub rsp, 0x8
                    cld
                    call {handler}
                    mov rdi, {interrupt_index}
                    call {end_interrupt}
                    add rsp, 0x8
                    ",
                    pop_registers!(),
                    "iretq",
                    handler = sym $handler,
                    regs_size = const size_of::<Registers>(),
                    interrupt_index = const $interrupt as u8,
                    end_interrupt = sym interrupt_return,
                    options(noreturn)
                )
            }
        }
        #[allow(unused_unsafe)]
        unsafe { $idt[$interrupt.as_usize()].set_handler_addr(VirtAddr::new(handler as u64)) }
    }};
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.divide_error.set_handler_fn(divide_error);
        idt.debug.set_handler_fn(debug);
        idt.non_maskable_interrupt
            .set_handler_fn(non_maskable_interrupt);
        idt.breakpoint.set_handler_fn(breakpoint);
        idt.overflow.set_handler_fn(overflow);
        idt.bound_range_exceeded
            .set_handler_fn(bound_range_exceeded);
        idt.invalid_opcode.set_handler_fn(invalid_opcode);
        idt.device_not_available
            .set_handler_fn(device_not_available);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt.invalid_tss.set_handler_fn(invalid_tss);
        idt.segment_not_present.set_handler_fn(segment_not_present);
        idt.stack_segment_fault.set_handler_fn(stack_segment_fault);
        idt.general_protection_fault
            .set_handler_fn(general_protection_fault);
        idt.page_fault.set_handler_fn(page_fault);
        idt.x87_floating_point.set_handler_fn(x87_floating_point);
        idt.alignment_check.set_handler_fn(alignment_check);
        idt.machine_check.set_handler_fn(machine_check);
        idt.simd_floating_point.set_handler_fn(simd_floating_point);
        idt.virtualization.set_handler_fn(virtualization);
        idt.vmm_communication_exception
            .set_handler_fn(vmm_communication_exception);
        idt.security_exception.set_handler_fn(security_exception);
        unsafe {
            register_interrupt!(idt, InterruptIndex::Timer => timer)
                .set_stack_index(gdt::TIMER_IST_INDEX);
        }
        register_interrupt!(idt, InterruptIndex::Keyboard => keyboard_interrupt_handler);

        idt
    };
}
pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

pub fn init() {
    IDT.load();
    unsafe { PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();
}

extern "x86-interrupt" fn divide_error(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION: DIVIDE BY ZERO\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn debug(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION: DEBUG\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn non_maskable_interrupt(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION: Non-Maskable Interrupt\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn breakpoint(_stack_frame: InterruptStackFrame) {
    #[cfg(not(test))]
    panic!("EXCEPTION: BREAKPOINT\n{:#?}", _stack_frame);
}

extern "x86-interrupt" fn overflow(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION: OVERFLOW\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn bound_range_exceeded(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION: BOUND RANGE EXCEEDED\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn invalid_opcode(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION: INVALID OPCODE\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn device_not_available(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION: DEVICE NOT AVAILABLE\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(stack_frame: InterruptStackFrame, _: u64) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{stack_frame:#?}");
}

extern "x86-interrupt" fn invalid_tss(stack_frame: InterruptStackFrame, error_code: u64) {
    panic!("EXCEPTION({error_code}): INVALID TSS\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn segment_not_present(stack_frame: InterruptStackFrame, error_code: u64) {
    panic!(
        "EXCEPTION({error_code}): SEGMENT NOT PRESENT\n{:#?}",
        stack_frame
    );
}

extern "x86-interrupt" fn stack_segment_fault(stack_frame: InterruptStackFrame, error_code: u64) {
    panic!(
        "EXCEPTION({error_code}): STACK SEGMENT FAULT\n{:#?}",
        stack_frame
    );
}

extern "x86-interrupt" fn general_protection_fault(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    panic!(
        "EXCEPTION({error_code}): GENERAL PROTECTION FAULT\n{:#?}",
        stack_frame
    );
}

extern "x86-interrupt" fn page_fault(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("Thread id: {}", current_thread().as_u64());
    println!("Exception: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error code: {error_code:?}");
    println!("{stack_frame:#?}");
    hlt_loop();
}

extern "x86-interrupt" fn x87_floating_point(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION: X87 FLOATING POINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn alignment_check(stack_frame: InterruptStackFrame, error_code: u64) {
    panic!(
        "EXCEPTION({error_code}): ALIGNMENT CHECK\n{:#?}",
        stack_frame
    );
}

extern "x86-interrupt" fn machine_check(stack_frame: InterruptStackFrame) -> ! {
    panic!("EXCEPTION: MACHINE CHECK\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn simd_floating_point(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION: SIMD FLOATING POINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn virtualization(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION: VIRTUALIZATION\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn vmm_communication_exception(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    panic!(
        "EXCEPTION({error_code}): VMM COMMUNICATION EXCEPTION\n{:#?}",
        stack_frame
    );
}

extern "x86-interrupt" fn security_exception(stack_frame: InterruptStackFrame, error_code: u64) {
    panic!(
        "EXCEPTION({error_code}): SECURITY EXCEPTION\n{:#?}",
        stack_frame
    );
}

extern "C" fn timer(stack_frame: &mut InterruptStackFrame, regs: &mut Registers) {
    if let Some(thread) = scheduler::schedule() {
        unsafe {
            stack_frame.as_mut().update(|frame| {
                //serial_println!(
                //    "context switching from {:?} to {:?}",
                //    current_thread(),
                //    thread.tid
                //);
                add_paused_thread(frame, regs, thread);
            });
        }
    }
}

extern "C" fn keyboard_interrupt_handler(
    _stack_frame: &mut InterruptStackFrame,
    _regs: &mut Registers,
) {
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    crate::task::keyboard::add_scancode(scancode);
}

extern "C" fn interrupt_return(interrupt: InterruptIndex) {
    unsafe { PICS.lock().notify_end_of_interrupt(interrupt.as_u8()) }
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex {
    const fn as_u8(self) -> u8 {
        self as u8
    }

    const fn as_usize(self) -> usize {
        self as usize
    }
}

#[cfg(test)]
mod tests {
    #[test_case]
    fn test_breakpoint_exception() {
        x86_64::instructions::interrupts::int3();
    }
}
