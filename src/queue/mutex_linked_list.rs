use queue::MPMCQueue;
use std::cell::{RefCell};
use std::ptr;
use std::boxed;
use std::ops::DerefMut;
use std::sync::{Mutex};

#[derive(Debug)]
struct Node<T> {
    value: Option<T>,
    next: RefCell<*mut Node<T>>,
}

impl<T> Node<T> {
    unsafe fn new(value: T) -> *mut Node<T> {
        boxed::into_raw(box Node {
            value: Some(value),
            next: RefCell::new(ptr::null_mut()),
        })
    }
}

#[derive(Debug)]
struct MutexLinkedList<T> {
    lock: Mutex<bool>,
    head: RefCell<*mut Node<T>>,
    tail: RefCell<*mut Node<T>>,
}

unsafe impl<T: Sync> Sync for MutexLinkedList<T> { }
unsafe impl<T: Send> Send for MutexLinkedList<T> { }

impl<T> MutexLinkedList<T> {
    pub fn new() -> MutexLinkedList<T> {
        // Initialize a stub ptr in order to correctly set up the tail and head ptrs.
        let stub = unsafe { boxed::into_raw(box Node {
            value: None, next: RefCell::new(ptr::null_mut())
        }) };
        MutexLinkedList {
            lock: Mutex::new(true),
            head: RefCell::new(stub),
            tail: RefCell::new(stub),
        }
    }
}

impl<T> MPMCQueue<T> for MutexLinkedList<T> {
    fn push(&self, value: T) {
        unsafe {
            let node = Node::new(value);

            let _lock = self.lock.lock();

            let prev = self.head.borrow().clone();
            *((*prev).next.borrow_mut().deref_mut()) = node;

            *(self.head.borrow_mut().deref_mut()) = node;
        }
    }

    fn pop(&self) -> Option<T> {
        unsafe {
            let _lock = self.lock.lock();
            let old = *(self.tail.borrow_mut().deref_mut());
            let next = *((*old).next.borrow_mut().deref_mut());

            if !next.is_null() {
                assert!((*old).value.is_none());
                assert!((*next).value.is_some());
                let ret = (*next).value.take().unwrap();
                *(self.tail.borrow_mut().deref_mut()) = next;
                let _: Box<Node<T>> = Box::from_raw(old);
                Some(ret)
            } else {
                None
            }
        }
    }
}

impl<T> Drop for MutexLinkedList<T> {
    fn drop(&mut self) {
        unsafe {
            let mut cur = *(self.head.borrow_mut().deref_mut());
            while !cur.is_null() {
                let next = *((*cur).next.borrow_mut().deref_mut());
                let _: Box<Node<T>> = Box::from_raw(cur);
                cur = next
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::MutexLinkedList;
    use queue::MPMCQueue;
    use std::sync::{Arc};
    use std::thread::scoped;

    #[test]
    fn test_push_pop() {
        let q = MutexLinkedList::new();
        assert!(q.pop().is_none());
        q.push(1);
        q.push(2);
        assert_eq!(q.pop().unwrap(), 1);
        assert_eq!(q.pop().unwrap(), 2);
    }

    #[test]
    fn test_concurrent() {
        let q = Arc::new(MutexLinkedList::new());
        let mut guard_vec = Vec::new();
        for i in 0..10 {
            let qu = q.clone();
            guard_vec.push(scoped(move || {
                qu.push(i as u8);
            }));
        }

        for x in guard_vec.into_iter() {
            x.join();
        }

        guard_vec = Vec::new();
        for _i in 0..10 {
            let qu = q.clone();
            guard_vec.push(scoped(move || {
                let popped = qu.pop().unwrap();
                let mut found = false;
                for x in 0..10 {
                    if popped == x {
                        found = true
                    }
                }
                assert!(found);
            }));
        }
    }
}
