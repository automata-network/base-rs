use std::prelude::v1::*;

use std::thread;
use std::time::Duration;

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

#[derive(Debug)]
pub struct PanicContext<T: std::fmt::Debug>(pub T);

impl<T: std::fmt::Debug> Drop for PanicContext<T> {
    fn drop(&mut self) {
        if thread::panicking() {
            glog::warn!("panicking context: {:?}", self.0)
        }
    }
}