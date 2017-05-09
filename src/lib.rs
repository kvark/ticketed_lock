/*!
Ticketed lock.

Ticketed lock is similar to RwLock, except that the acquisition of the lock is split:

  1. obtaining a ticket, which has to be done on the same thread as the locked storage
  2. waiting on a ticket, which puts the current thread to sleep until the ticket is due. That moment comes when all the previous tickets are processed.
  3. working with the data behind a read/lock guard
  4. when the guard is freed, it allows the following tickets to become active

A ticket can be moved between threads or even just lost.
Consecutive read-only tickets do not guarantee a particular lock order.
All the ticket counting is done based on `Arc` primitives, and the only unsafe code that this library has is for accessing the actual data behind a guard.

*/
#![warn(missing_docs)]

#[cfg(feature = "logging")]
#[macro_use]
extern crate log;
#[cfg(feature = "futuring")]
extern crate futures;

mod raw;

use std::{mem, ops};
use std::cell::UnsafeCell;
use std::sync::Arc;
#[cfg(feature = "futuring")]
use futures::{Async, Future, Poll};


/// The read-only guard of data, allowing `&T` dereferences.
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

/// A ticket to read the data at some point.
#[derive(Clone)]
pub struct ReadTicket<T> {
    inner: raw::Ticket<raw::Read>,
    data: Arc<UnsafeCell<T>>,
}

unsafe impl<T> Send for ReadTicket<T> {}

#[cfg(not(feature = "futuring"))]
impl<T> ReadTicket<T> {
    /// Wait for the ticket to become active, returning a lock guard.
    pub fn wait(self) -> ReadLockGuard<T> {
        ReadLockGuard {
            _inner: self.inner.wait(),
            data: self.data,
        }
    }
}

#[cfg(feature = "futuring")]
impl<T> Future for ReadTicket<T> {
    type Item = ReadLockGuard<T>;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.inner.check() {
            Some(guard) => Ok(Async::Ready(ReadLockGuard {
                _inner: guard,
                data: self.data.clone(),
            })),
            None => Ok(Async::NotReady),
        }
    }

    fn wait(self) -> Result<Self::Item, Self::Error> {
        Ok(ReadLockGuard {
            _inner: self.inner.wait(),
            data: self.data,
        })
    }
}


/// The read-only guard of data, allowing `&T` dereferences.
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

/// A ticket to read/write the data at some point.
pub struct WriteTicket<T> {
    inner: raw::Ticket<raw::Write>,
    data: Arc<UnsafeCell<T>>,
}

unsafe impl<T> Send for WriteTicket<T> {}

#[cfg(not(feature = "futuring"))]
impl<T> WriteTicket<T> {
    /// Wait for the ticket to become active, returning a lock guard.
    pub fn wait(self) -> WriteLockGuard<T> {
        WriteLockGuard {
            _inner: self.inner.wait(),
            data: self.data,
        }
    }
}

#[cfg(feature = "futuring")]
impl<T> Future for WriteTicket<T> {
    type Item = WriteLockGuard<T>;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.inner.check() {
            Some(guard) => Ok(Async::Ready(WriteLockGuard {
                _inner: guard,
                data: self.data.clone(),
            })),
            None => Ok(Async::NotReady),
        }
    }

    fn wait(self) -> Result<Self::Item, Self::Error> {
        Ok(WriteLockGuard {
            _inner: self.inner.wait(),
            data: self.data,
        })
    }
}


/// The ticketed lock, which wraps the data.
pub struct TicketedLock<T> {
    inner: raw::TicketedLock,
    data: Arc<UnsafeCell<T>>,
}

unsafe impl<T: Send + Sync> Send for TicketedLock<T> {}
unsafe impl<T: Send + Sync> Sync for TicketedLock<T> {}

impl<T> TicketedLock<T> {
    /// Create a new ticketed lock.
    pub fn new(data: T) -> TicketedLock<T> {
        TicketedLock {
            inner: raw::TicketedLock::new(),
            data: Arc::new(UnsafeCell::new(data)),
        }
    }

    /// Remove the lock and extract the data out.
    pub fn unlock(mut self) -> T {
        self.inner.flush();
        match Arc::try_unwrap(self.data) {
            Ok(data) => unsafe{ data.into_inner() },
            Err(_) => panic!("All the locks are supposed to be done after flush()"),
        }
    }

    /// Acquire a read-only ticket.
    pub fn read(&mut self) -> ReadTicket<T> {
        ReadTicket {
            inner: self.inner.read(),
            data: self.data.clone(),
        }
    }

    /// Acquire a write-only ticket.
    pub fn write(&mut self) -> WriteTicket<T> {
        WriteTicket {
            inner: self.inner.write(),
            data: self.data.clone(),
        }
    }
}
