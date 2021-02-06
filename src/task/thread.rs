use alloc::{
    alloc::{alloc, Layout},
    collections::{BTreeMap, VecDeque},
};
use core::sync::atomic::{AtomicU64, Ordering};
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::{VirtAddr, registers::rflags::RFlags};

lazy_static! {
    static ref THREADS: Mutex<BTreeMap<ThreadId, Thread>> = Mutex::new(BTreeMap::new());
    static ref THREAD_QUEUE: Mutex<VecDeque<ThreadId>> = Mutex::new(VecDeque::new());
}
static THREAD_ALIGN: usize = 16;
lazy_static! {
    static ref CURRENT_THREAD: Mutex<ThreadId> = Mutex::new(ThreadId::new());
}

pub struct Thread {
    tid: ThreadId,
    registers: Registers,
}

impl Thread {
    fn new(size: usize) -> ThreadId {
        let tid = ThreadId::new();
        let thread = Thread {
            tid,
            registers: Registers::init(size),
        };

        let mut map = THREADS.lock();
        map.insert(tid, thread);
        tid
    }

    pub fn switch_to(new_tid: ThreadId) {
        let current_tid = *CURRENT_THREAD.lock();
        if new_tid == current_tid {
            return;
        }

        // store registers with the thread
        let current_registers = unsafe { Registers::read() };
        let mut thread_map = THREADS.lock();
        let mut current = thread_map.get_mut(&current_tid).unwrap();
        current.registers = current_registers;

        // load registers from new and save them in cpu
        unsafe {
            thread_map.get(&new_tid).unwrap().registers.write();
        }

        // Do context switch
        todo!();
    }
}

#[derive(Debug)]
pub struct Registers {
    rip: VirtAddr,
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rsp: u64,
    rbp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rflags: RFlags,
}

impl Registers {
    fn init(size: usize) -> Self {
        Self {
            rsp: unsafe {
                *alloc(
                    Layout::from_size_align(size, THREAD_ALIGN).expect("Could not create stack!"),
                ) as u64
            },
            rip: VirtAddr::new(0), // TODO: load code segment/rip
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rflags: x86_64::registers::rflags::read(), // TODO: CHECK DEFAULTS
        }
    }

    pub unsafe fn read() -> Self {
        let rflags = x86_64::registers::rflags::read();
        let rax: u64;
        let rbx: u64;
        let rcx: u64;
        let rdx: u64;
        let rsi: u64;
        let rdi: u64;
        let rsp: u64;
        let rbp: u64;
        let r8: u64;
        let r9: u64;
        let r10: u64;
        let r11: u64;
        let r12: u64;
        let r13: u64;
        let r14: u64;
        let r15: u64;
        asm!(
            "mov {}, rax",
            "mov {}, rbx",
            "mov {}, rcx",
            "mov {}, rdx",
            "mov {}, rsi",
            "mov {}, rdi",
            "mov {}, rsp",
            "mov {}, rbp",
            "mov {}, r8",
            "mov {}, r9",
            "mov {}, r10",
            "mov {}, r11",
            "mov {}, r12",
            "mov {}, r13",
            "mov {}, r14",
            "mov {}, r15",
            out(reg) rax,
            out(reg) rbx,
            out(reg) rcx,
            out(reg) rdx,
            out(reg) rsi,
            out(reg) rdi,
            out(reg) rsp,
            out(reg) rbp,
            out(reg) r8,
            out(reg) r9,
            out(reg) r10,
            out(reg) r11,
            out(reg) r12,
            out(reg) r13,
            out(reg) r14,
            out(reg) r15,
            options(nostack),
        );
        Self {
            rip: x86_64::registers::read_rip(),
            rax,
            rbx,
            rcx,
            rdx,
            rsi,
            rdi,
            rsp,
            rbp,
            r8,
            r9,
            r10,
            r11,
            r12,
            r13,
            r14,
            r15,
            rflags,
        }
    }

    unsafe fn write(&self) {
        //asm!(
        //    "mov {}, rax",
        //    "mov {}, rbx",
        //    "mov {}, rcx",
        //    "mov {}, rdx",
        //    "mov {}, rsi",
        //    "mov {}, rdi",
        //    "mov {}, rsp",
        //    "mov {}, rbp",
        //    "mov {}, r8",
        //    "mov {}, r9",
        //    "mov {}, r10",
        //    "mov {}, r11",
        //    "mov {}, r12",
        //    "mov {}, r13",
        //    "mov {}, r14",
        //    "mov {}, r15",
        //    in(reg) self.rax,
        //    in(reg) self.rbx,
        //    in(reg) self.rcx,
        //    in(reg) self.rdx,
        //    in(reg) self.rsi,
        //    in(reg) self.rdi,
        //    in(reg) self.rsp,
        //    in(reg) self.rbp,
        //    in(reg) self.r8,
        //    in(reg) self.r9,
        //    in(reg) self.r10,
        //    in(reg) self.r11,
        //    in(reg) self.r12,
        //    in(reg) self.r13,
        //    in(reg) self.r14,
        //    in(reg) self.r15,
        //    options(nostack),
        //);
        //x86_64::registers::rflags::write(self.rflags);
        //asm!(
        //    "jmp {}",
        //    in(reg) self.rip.as_u64(),
        //);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThreadId(u64);

impl ThreadId {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        ThreadId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}
