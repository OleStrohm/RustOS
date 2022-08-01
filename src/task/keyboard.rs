use crate::println;
use alloc::sync::Arc;
use conquer_once::spin::OnceCell;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use crossbeam_queue::ArrayQueue;
use futures_util::task::AtomicWaker;
use futures_util::{stream::Stream, StreamExt};
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use spin::Mutex;

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
static KEYBOARD_LISTENERS: OnceCell<ArrayQueue<(Arc<Mutex<Option<DecodedKey>>>, Waker)>> =
    OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();

pub async fn recv() -> DecodedKey {
    let listener = KeyboardListener::new();
    let key = listener.await;

    match key {
        Some(key) => key,
        None => panic!("Keyboard error or something"),
    }
}

struct KeyboardListener {
    result: Arc<Mutex<Option<DecodedKey>>>,
}

impl KeyboardListener {
    pub fn new() -> Self {
        KeyboardListener {
            result: Arc::new(Mutex::new(None)),
        }
    }
}

impl Future for KeyboardListener {
    type Output = Option<DecodedKey>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(key) = *self.result.lock() {
            Poll::Ready(Some(key))
        } else {
            match KEYBOARD_LISTENERS.try_get() {
                Ok(queue) => match queue.push((self.result.clone(), cx.waker().clone())) {
                    Ok(()) => Poll::Pending,
                    Err(_) => Poll::Ready(None),
                },
                Err(_) => return Poll::Ready(None),
            }
        }
    }
}

pub struct ScancodeStream {
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE.init_once(|| ArrayQueue::new(100));
        ScancodeStream { _private: () }
    }
}

pub async fn keyboard_scheduler() {
    let mut scancodes = ScancodeStream::new();
    KEYBOARD_LISTENERS.init_once(|| ArrayQueue::new(100));

    let mut keyboard = Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore);

    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                let listener_queue = KEYBOARD_LISTENERS.try_get().unwrap();
                while let Some((result, waker)) = listener_queue.pop() {
                    let mut result = result.lock();
                    *result = Some(key);
                    waker.wake();
                }
            }
        }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let queue = SCANCODE_QUEUE
            .try_get()
            .expect("Scancode queue not initialized");

        if let Some(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }

        WAKER.register(&cx.waker());
        match queue.pop() {
            Some(scancode) => {
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            None => Poll::Pending,
        }
    }
}

pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            println!("WARNING: scancode queue full; dropping keyboard input");
        } else {
            WAKER.wake();
        }
    } else {
        println!("WARNING: scancode queue initialized");
    }
}
