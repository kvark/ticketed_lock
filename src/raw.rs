use std::sync::{Arc, Condvar, Mutex, Weak};

struct Link {
    cond_var: Condvar,
    mutex: Mutex<bool>,
    _index: usize,
}

struct Seal {
    link: Arc<Link>,
}

impl Drop for Seal {
    fn drop(&mut self) {
        *self.link.mutex.lock().unwrap() = true;
        self.link.cond_var.notify_all();
    }
}

type Legacy = Vec<Arc<Seal>>;

pub type Read = ();
pub struct Write;

#[derive(Clone)]
pub struct Ticket<A> {
    link: Arc<Link>,
    legacy: Arc<Mutex<Legacy>>,
    access: A,
}

pub struct LockGuard {
    _legacy: Arc<Mutex<Legacy>>,
}

impl<A> Ticket<A> {
    pub fn wait(self) -> LockGuard {
        // block until the seal is broken
        let mut lock = self.link.mutex.lock().unwrap();
        while !*lock {
            lock = self.link.cond_var.wait(lock).unwrap();
        }
        // transform to a guard
        LockGuard {
            _legacy: self.legacy,
        }
    }
}

pub struct TicketedLock {
    next_index: usize,
    legacies: Vec<Weak<Mutex<Legacy>>>,
    last_read: Option<Ticket<Read>>,
}

impl TicketedLock {
    pub fn new() -> TicketedLock {
        TicketedLock {
            next_index: 0,
            legacies: Vec::new(),
            last_read: None,
        }
    }

    fn issue(&mut self) -> (Arc<Link>, Arc<Mutex<Legacy>>) {
        // create a new link
        self.next_index += 1;
        let link = Arc::new(Link {
            cond_var: Condvar::new(),
            mutex: Mutex::new(false),
            _index: self.next_index,
        });
        // update all the existing legacies
        let seal = Arc::new(Seal {
            link: link.clone(),
        });
        self.legacies.retain(|legacy| {
            match legacy.upgrade() {
                Some(leg) => {
                    leg.lock().unwrap().push(seal.clone());
                    true
                },
                None => false,
            }
        });
        // add a new legacy
        let legacy = Arc::new(Mutex::new(Vec::new()));
        self.legacies.push(Arc::downgrade(&legacy));
        // done
        (link, legacy)
    }

    pub fn write(&mut self) -> Ticket<Write> {
        self.last_read = None;
        let (link, legacy) = self.issue();
        Ticket {
            link: link,
            legacy: legacy,
            access: Write,
        }
    }

    pub fn read(&mut self) -> Ticket<Read> {
        // check if there is already a read lock on top
        if let Some(ref ticket) = self.last_read {
            return ticket.clone();
        }
        let (link, legacy) = self.issue();
        Ticket {
            link: link,
            legacy: legacy,
            access: (),
        }
    }

    pub fn flush(&mut self) {
        //TODO
    }
}