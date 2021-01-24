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

#[cfg(feature = "log")]
#[macro_use]
extern crate log;
#[cfg(feature = "futures")]
extern crate futures;

mod raw;

use std::{mem, ops};
use std::cell::UnsafeCell;
use std::sync::Arc;
#[cfg(feature = "futures")]
use futures::{
    Future,
    task::{Context, Poll},
};
#[cfg(feature = "futures")]
use std::pin::Pin;


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

unsafe impl<T: Send> Send for ReadTicket<T> {}

impl<T> ReadTicket<T> {
    /// Wait for the ticket to become active, returning a lock guard.
    pub fn wait(self) -> Result<ReadLockGuard<T>, ()> {
        Ok(ReadLockGuard {
            _inner: self.inner.wait(),
            data: self.data,
        })
    }
}

#[cfg(feature = "futures")]
impl<T> Future for ReadTicket<T> {
    type Output = Result<ReadLockGuard<T>, ()>;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.inner.check() {
            Some(guard) => Poll::Ready(Ok(ReadLockGuard {
                _inner: guard,
                data: self.data.clone(),
            })),
            None => Poll::Pending,
        }
    }
}


/// The read/write guard of data, allowing `&mut T` dereferences.
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

unsafe impl<T: Send> Send for WriteTicket<T> {}

impl<T> WriteTicket<T> {
    /// Wait for the ticket to become active, returning a lock guard.
    pub fn wait(self) -> Result<WriteLockGuard<T>, ()> {
        Ok(WriteLockGuard {
            _inner: self.inner.wait(),
            data: self.data,
        })
    }
}

#[cfg(feature = "futures")]
impl<T> Future for WriteTicket<T> {
    type Output = Result<WriteLockGuard<T>, ()>;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.inner.check() {
            Some(guard) => Poll::Ready(Ok(WriteLockGuard {
                _inner: guard,
                data: self.data.clone(),
            })),
            None => Poll::Pending,
        }
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
            Ok(data) => data.into_inner(),
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

    /// Acquire a read/write ticket.
    pub fn write(&mut self) -> WriteTicket<T> {
        WriteTicket {
            inner: self.inner.write(),
            data: self.data.clone(),
        }
    }
}
