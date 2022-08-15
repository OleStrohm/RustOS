use super::thread::{Registers, Thread, ThreadId};
use crate::memory::{lock_frame_allocator, lock_memory_mapper};
use alloc::collections::{BTreeMap, VecDeque};
use core::sync::atomic::{AtomicU64, Ordering};
use spin::{Mutex, Once};
use x86_64::structures::idt::InterruptStackFrameValue;

static SCHEDULER: Once<Mutex<Scheduler>> = Once::new();
static CURRENT_THREAD: AtomicU64 = AtomicU64::new(0);

pub fn init_scheduler() {
    SCHEDULER.call_once(|| Mutex::new(Scheduler::new()));
}

pub struct Scheduler {
    threads: BTreeMap<ThreadId, Thread>,
    queue: VecDeque<ThreadId>,
}

impl Scheduler {
    fn new() -> Self {
        let root_thread = Thread::create_root_thread();
        let root_id = root_thread.tid;
        let threads = BTreeMap::from([(root_id, root_thread)]);

        Scheduler {
            threads,
            queue: VecDeque::default(),
        }
    }

    fn schedule(&mut self) -> Option<Thread> {
        self.queue
            .pop_front()
            .and_then(|tid| self.threads.get(&tid).copied())
    }

    fn register_thread(&mut self, thread: Thread) {
        let prev = self.threads.insert(thread.tid, thread);
        if prev.is_some() {
            panic!("Thread with id {} already exists", thread.tid.as_u64());
        }
        self.queue.push_back(thread.tid);
    }
}

pub fn spawn_user(entrypoint: fn() -> !) {
    let mut mapper = lock_memory_mapper();
    let mut frame_allocator = lock_frame_allocator();

    let thread =
        Thread::create_userspace_entrypoint(&mut *mapper, &mut *frame_allocator, entrypoint);
    SCHEDULER.get().unwrap().lock().register_thread(thread);
}

pub fn spawn(entrypoint: fn() -> !) {
    let mut mapper = lock_memory_mapper();
    let mut frame_allocator = lock_frame_allocator();

    let thread = Thread::create_closure(&mut *mapper, &mut *frame_allocator, entrypoint);
    SCHEDULER.get().unwrap().lock().register_thread(thread);
}

pub fn current_thread() -> ThreadId {
    unsafe { ThreadId::from_u64(CURRENT_THREAD.load(Ordering::SeqCst)) }
}

pub fn schedule() -> Option<Thread> {
    SCHEDULER.get()?.try_lock()?.schedule()
}

pub fn add_paused_thread(
    stack_frame: &mut InterruptStackFrameValue,
    regs: &mut Registers,
    thread: Thread,
) {
    let mut scheduler = SCHEDULER.get().unwrap().lock();

    let current_tid =
        unsafe { ThreadId::from_u64(CURRENT_THREAD.swap(thread.tid.as_u64(), Ordering::SeqCst)) };
    let current_thread = scheduler.threads.get_mut(&current_tid).unwrap();
    current_thread.stack_frame.replace(stack_frame.clone());
    current_thread.regs.replace(regs.clone());

    let new_thread = scheduler.threads.get_mut(&thread.tid).unwrap();
    *stack_frame = new_thread.stack_frame.take().unwrap();
    *regs = new_thread.regs.take().unwrap();
    scheduler.queue.push_back(current_tid);
}
