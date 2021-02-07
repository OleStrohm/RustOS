use alloc::boxed::Box;
use alloc::{
    alloc::{alloc, Layout},
    collections::{BTreeMap, VecDeque},
};
use core::sync::atomic::{AtomicU64, Ordering};
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::VirtAddr;

lazy_static! {
    static ref THREADS: Mutex<BTreeMap<ThreadId, Thread>> = Mutex::new(BTreeMap::new());
    static ref THREAD_QUEUE: Mutex<VecDeque<ThreadId>> = Mutex::new(VecDeque::new());
}
static THREAD_ALIGN: usize = 4096;
lazy_static! {
    static ref CURRENT_THREAD: Mutex<ThreadId> = Mutex::new(ThreadId::new());
}

#[derive(Debug)]
pub struct Thread {
    tid: ThreadId,
    stack: Option<Stack>,
    stack_bounds: Option<StackBounds>,
}

impl Thread {
    pub fn spawn(entrypoint: Box<dyn Fn() -> !>) {
        let thread = Thread::new(4096 * 10);
        THREADS.lock().insert(thread.tid, thread);
    }

    pub(super) unsafe fn create_root_thread() {
        Thread {
            tid: ThreadId(0),
            stack: None,
            stack_bounds: None,
        };
    }

    fn new(size: usize) -> Thread {
        let stack = Stack::allocate(size);
        Thread {
            tid: ThreadId::new(),
            stack: Some(stack),
            stack_bounds: Some(StackBounds::from_stack_size(stack, size)),
        }
    }

    pub unsafe fn switch_to(new_tid: ThreadId, current_stack: Stack) {
        let current_tid = *CURRENT_THREAD.lock();
        if new_tid == current_tid {
            return;
        }

        // store registers with the thread
        let mut thread_map = THREADS.lock();
        let mut current = thread_map.get_mut(&new_tid).unwrap();
        current.stack = Some(current_stack);
        let new = thread_map.get(&new_tid).unwrap();

        crate::gdt::context_switch_to(new_tid, new.stack.unwrap());
    }
}

#[derive(Debug)]
pub struct Stack {
    rsp: VirtAddr,
}

impl Stack {
    pub unsafe fn new(rsp: VirtAddr) -> Self {
        Stack { rsp }
    }

    pub fn allocate(size: usize) -> Self {
        unsafe {
            Stack::new(VirtAddr::new(
                alloc(Layout::from_size_align(size, THREAD_ALIGN).unwrap()) as u64,
            ))
        }
    }

    pub fn get_rsp(&self) -> VirtAddr {
        self.rsp
    }

    pub fn setup_for_entry() {}

    unsafe fn push<T>(&mut self, value: T) {
        self.rsp -= core::mem::size_of::<T>();
        let ptr: *mut T = self.rsp.as_mut_ptr();
        ptr.write(value);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThreadId(u64);

impl ThreadId {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        ThreadId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl Into<u64> for ThreadId {
    fn into(self) -> u64 {
        let ThreadId(inner) = self;
        inner
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StackBounds(VirtAddr, VirtAddr);

impl StackBounds {
    fn from_stack_size(stack: Stack, size: usize) -> Self {
        StackBounds(stack.get_rsp() - size, stack.get_rsp())
    }
}
