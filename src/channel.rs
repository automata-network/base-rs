use std::prelude::v1::*;

use std::sync::mpsc::TrySendError;
use std::sync::{mpsc, Arc, Mutex};

#[derive(Clone, Debug)]
pub struct Boardcast<T: Clone> {
    senders: Arc<Mutex<Vec<mpsc::Sender<T>>>>,
    latest: Arc<Mutex<Option<T>>>,
}

impl<T: Clone> Boardcast<T> {
    pub fn new() -> Self {
        Self {
            senders: Default::default(),
            latest: Default::default(),
        }
    }

    pub fn new_with(t: T) -> Self {
        let bcast = Self::new();
        bcast.boardcast(t);
        bcast
    }

    pub fn new_subscriber(&self) -> mpsc::Receiver<T> {
        let mut senders = self.senders.lock().unwrap();
        let (sender, receiver) = mpsc::channel();
        senders.push(sender);
        receiver
    }

    pub fn get_latest(&self) -> Option<T> {
        let latest = self.latest.lock().unwrap();
        latest.as_ref().map(|item| item.clone())
    }

    pub fn len(&self) -> usize {
        self.senders.lock().unwrap().len()
    }

    pub fn boardcast(&self, item: T) {
        {
            let mut senders = self.senders.lock().unwrap();
            let mut idx = 0;
            while idx < senders.len() {
                if let Err(_) = senders[idx].send(item.clone()) {
                    senders.remove(idx);
                    continue;
                }
                idx += 1;
            }
        }
        {
            let mut latest = self.latest.lock().unwrap();
            *latest = Some(item.clone());
        }
    }

    pub fn clean(&self) {
        let mut senders = self.senders.lock().unwrap();
        let mut tmp = Vec::new();
        std::mem::swap(&mut tmp, &mut senders);
    }
}

pub struct Dispatcher<T> {
    senders: Mutex<Vec<mpsc::SyncSender<T>>>,
}

impl<T> Dispatcher<T> {
    pub fn new() -> Self {
        Self {
            senders: Mutex::new(Vec::new()),
        }
    }

    pub fn dispatch(&self, mut t: T) -> Option<T> {
        let mut senders = self.senders.lock().unwrap();
        let mut idx = 0;
        while idx < senders.len() {
            match senders[idx].try_send(t) {
                Ok(_) => {
                    return None;
                }
                Err(TrySendError::Full(obj)) => {
                    t = obj;
                    idx += 1;
                }
                Err(TrySendError::Disconnected(obj)) => {
                    t = obj;
                    senders.remove(idx);
                }
            }
        }
        Some(t)
    }

    pub fn subscribe(&self) -> mpsc::Receiver<T> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.senders.lock().unwrap().push(sender);
        receiver
    }
}
