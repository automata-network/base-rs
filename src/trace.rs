use std::prelude::v1::*;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc, Arc,
};
use std::time::{Duration, Instant};

use std::collections::BTreeMap;

use crate::time::{SignedDuration, Time};

#[derive(Clone)]
pub struct Alive {
    alive: Arc<AtomicBool>,
    parent: Box<Option<Alive>>,
    deadline: Option<Time>,
}

impl Default for Alive {
    fn default() -> Self {
        Self::new()
    }
}

impl Alive {
    pub fn new() -> Self {
        Self {
            alive: Arc::new(AtomicBool::new(true)),
            parent: Box::new(None),
            deadline: None,
        }
    }

    pub fn deadline(&self) -> Option<Time> {
        self.deadline
    }

    pub fn remain_time(&self) -> Option<SignedDuration> {
        self.deadline.map(|item| item.duration_since(Time::now()))
    }

    pub fn is_alive(&self) -> bool {
        if !self.alive.load(Ordering::Relaxed) {
            return false;
        }
        if let Some(deadline) = self.deadline {
            if Time::now() >= deadline {
                return false;
            }
        }
        if let Some(parent) = self.parent.as_ref() {
            return parent.is_alive();
        }
        return true;
    }

    pub fn shutdown(&self) {
        self.alive.store(false, Ordering::Relaxed);
    }

    pub fn with_deadline(&mut self, deadline: Time) -> &mut Self {
        self.deadline = Some(deadline);
        self
    }

    pub fn fork_with_timeout(&self, dur: Duration) -> Self {
        self.fork_with_deadline(Time::now() + dur)
    }

    pub fn fork_with_deadline(&self, deadline: Time) -> Self {
        Self {
            alive: Arc::new(AtomicBool::new(true)),
            parent: Box::new(Some(self.clone())),
            deadline: Some(match self.deadline {
                Some(d) => d.min(deadline),
                None => deadline,
            }),
        }
    }

    pub fn fork(&self) -> Alive {
        Self {
            alive: Arc::new(AtomicBool::new(true)),
            parent: Box::new(Some(self.clone())),
            deadline: self.deadline,
        }
    }

    pub fn sleep_ms(&self, ms: u64) -> bool {
        self.sleep(Duration::from_millis(ms))
    }

    pub fn sleep(&self, dur: Duration) -> bool {
        self.sleep_to(Time::now() + dur);
        self.is_alive()
    }

    pub fn sleep_to(&self, deadline: Time) {
        let max_sleep = Duration::from_secs(1);
        loop {
            if !self.is_alive() {
                break;
            }
            let now = Time::now();
            if now >= deadline {
                break;
            }
            let dur = (deadline - now).min(max_sleep);
            std::thread::sleep(dur);
        }
    }

    pub fn recv<T>(&self, r: &mpsc::Receiver<T>) -> Result<T, mpsc::RecvTimeoutError> {
        let max_sleep = Duration::from_secs(1);
        loop {
            if !self.is_alive() {
                break;
            }
            let mut timeout = max_sleep;
            if let Some(t) = self.remain_time() {
                if let Some(t) = t.duration() {
                    if t < timeout {
                        timeout = t;
                    }
                }
            }
            match r.recv_timeout(timeout) {
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                other => return other,
            }
        }

        return Err(mpsc::RecvTimeoutError::Timeout);
    }

    pub fn recv_iter<'a, T>(
        &'a self,
        r: &'a mpsc::Receiver<T>,
        poll: Duration,
    ) -> AliveIter<T, RecvIter<'a, T>> {
        self.iter(RecvIter {
            alive: self,
            dur: poll,
            receiver: r,
        })
    }

    pub fn iter<N, I, II>(&self, n: N) -> AliveIter<I, II>
    where
        N: IntoIterator<Item = I, IntoIter = II>,
        II: Iterator<Item = I>,
    {
        let iter = n.into_iter();
        AliveIter { alive: self, iter }
    }
}

pub struct RecvIter<'a, T> {
    alive: &'a Alive,
    dur: Duration,
    receiver: &'a mpsc::Receiver<T>,
}

impl<'a, T> Iterator for RecvIter<'a, T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.receiver.recv_timeout(self.dur) {
                Ok(n) => return Some(n),
                Err(mpsc::RecvTimeoutError::Disconnected) => return None,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !self.alive.is_alive() {
                        return None;
                    }
                    continue;
                }
            }
        }
    }
}

pub struct AliveIter<'a, T, I: Iterator<Item = T>> {
    alive: &'a Alive,
    iter: I,
}

impl<'a, T, I: Iterator<Item = T>> Iterator for AliveIter<'a, T, I> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.alive.is_alive() {
            return None;
        }
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.iter.count()
    }
}

impl std::fmt::Debug for Alive {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", if self.is_alive() { "ALIVE" } else { "DEAD" })
    }
}

#[derive(Clone, Default, Debug)]
pub struct AvgCounterResult {
    pub cnt: usize,
    pub total: Duration,
}

impl AvgCounterResult {
    pub fn new() -> Self {
        Self::default()
    }
}

impl std::fmt::Display for AvgCounterResult {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.cnt == 0 {
            write!(f, "0ns")
        } else {
            write!(
                f,
                "{:?}*{}={:?}",
                self.total / self.cnt as u32,
                self.cnt,
                self.total,
            )
        }
    }
}

#[derive(Clone, Debug)]
pub struct AvgCounter {
    cnt: Arc<AtomicUsize>,
    ms: Arc<AtomicUsize>,
}

impl AvgCounter {
    pub fn new() -> Self {
        Self {
            cnt: Arc::new(AtomicUsize::new(0)),
            ms: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn take(&self) -> AvgCounterResult {
        let cnt = self.cnt.swap(0, Ordering::SeqCst);
        let total = self.ms.swap(0, Ordering::SeqCst);
        AvgCounterResult {
            cnt,
            total: Duration::from_millis(total as u64),
        }
    }

    pub fn place(&self) -> AvgCounterGuard {
        AvgCounterGuard {
            raw: self,
            start: Instant::now(),
        }
    }
}

pub struct AvgCounterGuard<'a> {
    raw: &'a AvgCounter,
    start: Instant,
}

impl<'a> Drop for AvgCounterGuard<'a> {
    fn drop(&mut self) {
        self.raw.cnt.fetch_add(1, Ordering::SeqCst);
        let ms = self.start.elapsed().as_millis() as usize;
        self.raw.ms.fetch_add(ms, Ordering::SeqCst);
    }
}

pub struct Slowlog<'a>(&'a str, Instant, Duration);

impl<'a> Slowlog<'a> {
    pub fn new(tag: &'a str, du: Duration) -> Self {
        Self(tag, Instant::now(), du)
    }
    pub fn new_ms(tag: &'a str, du: u64) -> Self {
        Self::new(tag, Duration::from_millis(du))
    }
}

impl<'a> Drop for Slowlog<'a> {
    fn drop(&mut self) {
        let elapsed = self.1.elapsed();
        if elapsed > self.2 {
            glog::warn!("[slowlog] [{}] tooks {:?}", self.0, elapsed);
        }
    }
}

pub struct ItemIndexer<T>(pub BTreeMap<T, usize>);

impl<T: Clone + Ord> ItemIndexer<T> {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn index(&mut self, item: &T) -> usize {
        match self.0.get(item) {
            Some(item) => *item,
            None => {
                let idx = self.0.len();
                self.0.insert(item.clone(), idx);
                idx
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Counter(Arc<AtomicUsize>);

impl Counter {
    pub fn new() -> Self {
        Self(Default::default())
    }

    pub fn add(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    pub fn take(&self) -> usize {
        self.0.swap(0, Ordering::SeqCst)
    }
}
