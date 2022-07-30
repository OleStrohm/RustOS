use super::thread::{Registers, Thread, ThreadId};
use crate::memory::BootInfoFrameAllocator;
use alloc::collections::{BTreeMap, VecDeque};
use core::sync::atomic::{AtomicU64, Ordering};
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::structures::idt::InterruptStackFrameValue;
use x86_64::structures::paging::OffsetPageTable;

lazy_static! {
    pub static ref SCHEDULER: Mutex<Option<Scheduler>> = Mutex::new(None);
    static ref MAPPER: Mutex<Option<OffsetPageTable<'static>>> = Mutex::new(None);
    static ref FRAME_ALLOCATOR: Mutex<Option<BootInfoFrameAllocator>> = Mutex::new(None);
}
static CURRENT_THREAD: AtomicU64 = AtomicU64::new(0);

pub fn init_scheduler(mapper: OffsetPageTable<'static>, frame_allocator: BootInfoFrameAllocator) {
    *SCHEDULER.lock() = Some(Scheduler::new());
    *MAPPER.lock() = Some(mapper);
    *FRAME_ALLOCATOR.lock() = Some(frame_allocator);
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

pub fn spawn(entrypoint: fn() -> !) {
    let mut mapper = MAPPER.lock();
    let mut frame_allocator = FRAME_ALLOCATOR.lock();

    let thread = Thread::create_entrypoint(
        mapper.as_mut().unwrap(),
        frame_allocator.as_mut().unwrap(),
        entrypoint,
    );
    let mut scheduler = SCHEDULER.lock();
    scheduler.as_mut().unwrap().register_thread(thread);
}

pub fn current_thread() -> ThreadId {
    unsafe { ThreadId::from_u64(CURRENT_THREAD.load(Ordering::SeqCst)) }
}

pub fn schedule() -> Option<Thread> {
    let next = SCHEDULER
        .try_lock()
        .and_then(|mut s| s.as_mut().and_then(|s| s.schedule()));

    next
}

pub fn add_paused_thread(
    stack_frame: &mut InterruptStackFrameValue,
    regs: &mut Registers,
    thread: Thread,
) {
    let mut scheduler = SCHEDULER.lock();
    let scheduler = scheduler.as_mut().unwrap();
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
