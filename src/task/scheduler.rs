use super::thread::{Registers, Thread, ThreadId};
use crate::memory::BootInfoFrameAllocator;
use alloc::collections::{BTreeMap, VecDeque};
use core::sync::atomic::{AtomicU64, Ordering};
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::structures::idt::InterruptStackFrameValue;
use x86_64::structures::paging::OffsetPageTable;

lazy_static! {
    pub static ref SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::empty());
    static ref MAPPER: Mutex<Option<OffsetPageTable<'static>>> = Mutex::new(None);
    static ref FRAME_ALLOCATOR: Mutex<Option<BootInfoFrameAllocator>> = Mutex::new(None);
}
static CURRENT_THREAD: AtomicU64 = AtomicU64::new(0);

pub fn init_scheduler(mapper: OffsetPageTable<'static>, frame_allocator: BootInfoFrameAllocator) {
    *SCHEDULER.lock() = Scheduler::new();
    *MAPPER.lock() = Some(mapper);
    *FRAME_ALLOCATOR.lock() = Some(frame_allocator);
}

pub struct Scheduler {
    threads: BTreeMap<ThreadId, Thread>,
    queue: VecDeque<ThreadId>,
    enabled: bool,
}

impl Scheduler {
    fn empty() -> Self {
        Scheduler {
            threads: Default::default(),
            queue: Default::default(),
            enabled: false,
        }
    }

    fn new() -> Self {
        let root_thread = Thread::create_root_thread();
        let root_id = root_thread.tid;
        let threads = BTreeMap::from([(root_id, root_thread)]);

        Scheduler {
            threads,
            queue: VecDeque::default(),
            enabled: true,
        }
    }

    fn schedule(&mut self) -> Option<Thread> {
        self.enabled
            .then(|| {
                self.queue
                    .pop_front()
                    .and_then(|tid| self.threads.get(&tid).copied())
            })
            .flatten()
    }

    pub fn add_paused_thread(
        &mut self,
        stack_frame: &mut InterruptStackFrameValue,
        regs: &mut Registers,
        thread: Thread,
    ) {
        let current_tid = unsafe {
            ThreadId::from_u64(CURRENT_THREAD.swap(thread.tid.as_u64(), Ordering::SeqCst))
        };
        let current_thread = self.threads.get_mut(&current_tid).unwrap();
        current_thread.stack_frame.replace(stack_frame.clone());
        current_thread.regs.replace(regs.clone());

        let new_thread = self.threads.get_mut(&thread.tid).unwrap();
        *stack_frame = new_thread.stack_frame.take().unwrap();
        *regs = new_thread.regs.take().unwrap();
        self.queue.push_back(current_tid);
    }

    fn register_thread(&mut self, thread: Thread) {
        let prev = self.threads.insert(thread.tid, thread);
        if prev.is_some() {
            panic!("Thread with id {} already exists", thread.tid.as_u64());
        }
        self.queue.push_back(thread.tid);
    }
}

pub fn spawn(entrypoint: fn() -> !) {
    let mut mapper = MAPPER.lock();
    let mut frame_allocator = FRAME_ALLOCATOR.lock();

    let thread = Thread::create_entrypoint(
        mapper.as_mut().unwrap(),
        frame_allocator.as_mut().unwrap(),
        entrypoint,
    );
    let mut scheduler = SCHEDULER.lock();
    scheduler.register_thread(thread);
}

pub fn current_thread() -> ThreadId {
    unsafe { ThreadId::from_u64(CURRENT_THREAD.load(Ordering::SeqCst)) }
}

pub fn schedule() -> Option<Thread> {
    let next = SCHEDULER.try_lock().and_then(|mut s| s.schedule());
    next
}

//fn switch_to(new: Thread) {
//    //switch_thread(new.tid, new.stack_frame.unwrap());
//}
//
//global_asm!(
//    "
//    asm_context_switch:
//        pushfq
//
//        mov rax, rsp
//        mov rsp, rdi
//
//        mov rdi, rax
//        call add_paused_thread
//return_context:
//
//        popfq
//        ret
//"
//);
//
//fn switch_thread(thread: ThreadId, stack: VirtAddr) {
//    let stack = stack.as_u64();
//    let thread: u64 = thread.into();
//    unsafe {
//        asm!(
//            "call asm_context_switch",
//            in("rdi") stack, in("rsi") thread,
//            //clobber_abi("C"),
//            //"rbx", "rcx", "rdx", "rsi", "rdi", "rpb", "r8", "r9",
//            //"r10", "r11", "r12", "r13", "r14", "r15", "rflags"
//        );
//    }
//}
//
//#[no_mangle]
//pub extern "C" fn add_paused_thread(current_stack: VirtAddr, new_tid: ThreadId) {
//    let mut scheduler = SCHEDULER.lock();
//    let current_tid = mem::replace(&mut scheduler.current_thread, new_tid);
//    //scheduler
//    //    .threads
//    //    .get_mut(&current_tid)
//    //    .unwrap()
//    //    .rsp
//    //    .replace(current_stack);
//    scheduler.queue.push_back(current_tid);
//    unsafe {
//        asm!(
//            "
//            jmp return_context
//        "
//        )
//    }
//}
