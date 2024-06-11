use std::thread::{self, JoinHandle};

pub enum AsyncResource<T> {
    Pending(Option<JoinHandle<T>>),
    Finished(T),
    Error(String)
}

impl<T> AsyncResource<T> {
    pub fn new<F: FnOnce() -> T>(f: F) -> Self {
        Self::Pending(Some(thread::spawn(f)))
    }

    pub fn poll(&mut self) -> Self {
        match self {
            AsyncResource::Pending(t) => {
                if t.is_finished() {
                    match t.take().unwrap().join() {
                        Ok(v) => *self = AsyncResource::Finished(v),
                        Err(e) => *self = AsyncResource::Error(format!("{:?}", e)),
                    }
                }
            },
            _ => (),
        }
    }

    pub fn is_pending(&self) -> bool {
        match self {
            AsyncResource::Pending(_) => true,
            _ => false,
        }
    }

    pub fn is_finished(&self) -> bool {
        match self {
            AsyncResource::Finished(_) => true,
            AsyncResource::Error(_) => true,
        }
    }

    pub fn get_result(&self) -> Option<&T> {
        match self {
            AsyncResource::Finished(t) => Some(t),
            _ => None
        }
    }
}