use std::prelude::v1::*;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::channel::Dispatcher;
use crate::trace::Alive;

pub fn spawn<F, T>(name: String, f: F) -> thread::JoinHandle<T>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Send + 'static,
{
    thread::Builder::new()
        .name(name)
        .spawn(f)
        .expect("failed to spawn thread")
}

pub fn sleep_ms(ms: u64) {
    thread::sleep(Duration::from_millis(ms));
}

pub fn join<T>(handle: &mut Option<thread::JoinHandle<T>>) {
    handle.take().map(|handle| {
        glog::info!("joining {:?}", handle.thread().name());
        handle.join()
    });
}

pub struct DeferFunc<F: Fn()>(F);
impl<F: Fn()> Drop for DeferFunc<F> {
    fn drop(&mut self) {
        self.0();
    }
}

#[derive(Debug)]
pub struct PanicContext<T: std::fmt::Debug>(pub T);

impl<T: std::fmt::Debug> Drop for PanicContext<T> {
    fn drop(&mut self) {
        if thread::panicking() {
            glog::warn!("panicking context: {:?}", self.0)
        }
    }
}

pub fn parallel<T, F>(alive: &Alive, tasks: Vec<T>, worker: usize, f: F) -> usize
where
    T: Send + 'static + Clone + std::fmt::Debug,
    F: Fn(T) -> Result<(), String> + Send + 'static + Clone,
{
    let alive = alive.fork();
    let dispatcher = <Dispatcher<T>>::new();
    let mut handles = Vec::with_capacity(worker);
    let processed = Arc::new(AtomicUsize::new(0));
    for i in 0..worker {
        let handle = spawn(format!("parallel-worker-{}", i), {
            let handler = f.clone();
            let receiver = dispatcher.subscribe();
            let alive = alive.clone();
            let processed = processed.clone();
            move || {
                let _defer = DeferFunc(|| {
                    if thread::panicking() {
                        alive.shutdown()
                    }
                });
                for item in alive.recv_iter(&receiver, Duration::from_millis(100)) {
                    if let Err(err) = handler(item.clone()) {
                        glog::error!("parallel execution fail: task:{:?}, info: {}", item, err);
                        alive.shutdown();
                    }
                    processed.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
        handles.push(handle);
    }
    for task in alive.iter(tasks) {
        let mut result = dispatcher.dispatch(task.clone());
        loop {
            match result {
                Some(task) => {
                    if !alive.sleep_ms(100) {
                        break;
                    }
                    result = dispatcher.dispatch(task);
                }
                None => break,
            }
        }
    }
    dispatcher.close_write();
    for handle in handles {
        let _ = handle.join();
    }
    return processed.load(Ordering::SeqCst);
}
