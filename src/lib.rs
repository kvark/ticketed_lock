mod raw;

use std::{mem, ops};
use std::cell::UnsafeCell;
use std::sync::Arc;


pub struct ReadLockGuard<T> {
    _inner: raw::LockGuard,
    data: Arc<UnsafeCell<T>>,
}

impl<T> ops::Deref for ReadLockGuard<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe{ mem::transmute(self.data.get()) }
    }
}

#[derive(Clone)]
pub struct ReadTicket<T> {
    inner: raw::Ticket<raw::Read>,
    data: Arc<UnsafeCell<T>>,
}

unsafe impl<T> Send for ReadTicket<T> {}

impl<T> ReadTicket<T> {
    pub fn wait(self) -> ReadLockGuard<T> {
        ReadLockGuard {
            _inner: self.inner.wait(),
            data: self.data,
        }
    }
}


pub struct WriteLockGuard<T> {
    _inner: raw::LockGuard,
    data: Arc<UnsafeCell<T>>,
}

impl<T> ops::Deref for WriteLockGuard<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { mem::transmute(self.data.get()) }
    }
}

impl<T> ops::DerefMut for WriteLockGuard<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { mem::transmute(self.data.get()) }
    }
}

pub struct WriteTicket<T> {
    inner: raw::Ticket<raw::Write>,
    data: Arc<UnsafeCell<T>>,
}

unsafe impl<T> Send for WriteTicket<T> {}

impl<T> WriteTicket<T> {
    pub fn wait(self) -> WriteLockGuard<T> {
        WriteLockGuard {
            _inner: self.inner.wait(),
            data: self.data,
        }
    }
}


pub struct TicketedLock<T> {
    inner: raw::TicketedLock,
    data: Arc<UnsafeCell<T>>,
}

impl<T> TicketedLock<T> {
    pub fn new(data: T) -> TicketedLock<T> {
        TicketedLock {
            inner: raw::TicketedLock::new(),
            data: Arc::new(UnsafeCell::new(data)),
        }
    }

    pub fn unlock(mut self) -> T {
        self.inner.flush();
        match Arc::try_unwrap(self.data) {
            Ok(data) => unsafe{ data.into_inner() },
            Err(_) => panic!("All the locks are supposed to be done after flush()"),
        }
    }

    pub fn read(&mut self) -> ReadTicket<T> {
        ReadTicket {
            inner: self.inner.read(),
            data: self.data.clone(),
        }
    }

    pub fn write(&mut self) -> WriteTicket<T> {
        WriteTicket {
            inner: self.inner.write(),
            data: self.data.clone(),
        }
    }
}
