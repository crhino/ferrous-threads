//! This module contains an implementation of a thread pool that will automatically spin up a
//! configurable amount of OS threads on start-up, and will only allow a configurable amount of
//! total OS threads.
//!
//! ```
//! #![feature(box_syntax)]
//! #![feature(result_expect)]
//! use std::sync::mpsc::channel;
//! use ferrous_threads::thread_pool::ThreadPool;
//!
//! let mut pool = ThreadPool::new(1, 2);
//! let thread = pool.thread().unwrap();
//!
//! let (sn, rc) = channel();
//! thread.start(box move || { sn.send(9u8).unwrap();}).expect("Could not send Proc");
//! assert!(thread.join().is_ok());
//! assert!(rc.recv().unwrap() == 9u8);
//! ```
//!
//! If a job running on a thread panics, the pool will detect this and spawn a new thread to take
//! the place of the old one. The user of the pool is able to detect this error condition through
//! the result returned on join().
//!
//! ```
//! #![feature(box_syntax)]
//! #![feature(result_expect)]
//! use std::sync::mpsc::channel;
//! use ferrous_threads::thread_pool::ThreadPool;
//!
//! let mut pool = ThreadPool::new(1, 2);
//! let _ = pool.thread().unwrap();
//! let thread = pool.thread().unwrap();
//!
//! thread.start(box move || { panic!();}).expect("Could not send Proc");
//! assert!(thread.join().is_err());
//! ```

use std::boxed::FnBox;
use std::thread::{self, spawn};
use std::sync::mpsc::{channel, Sender, SendError, Receiver, TryRecvError, RecvError};
use std::error::Error;
use std::fmt;

trait Runner {
    fn run(self: Box<Self>);
}

impl <F: FnBox()> Runner for F {
    fn run(self: Box<F>) {
        self.call_box(())
    }
}

/// Type used by ThreadPool to run closures, etc. on the spawned threads.
pub type Proc<'a> = Box<Runner + Send + 'a>;

/// Used to interact with the spawned threads returned by the ThreadPool.
///
/// See documentation for ThreadPool for more information.
pub struct Thread {
    inner: ThreadInner,
}

struct ThreadInner {
    sender: Sender<Proc<'static>>,
    result: Receiver<thread::Result<()>>,
}

impl Thread {
    fn new(sender: Sender<Proc<'static>>, result: Receiver<thread::Result<()>>) -> Thread {
        Thread {
            inner: ThreadInner {
                       sender: sender,
                       result: result,
                   },
        }
    }

    /// Sends the runnable object to a spawned thread to be run.
    pub fn start(&self, f: Proc<'static>) -> Result<(), SendError<Proc<'static>>> {
        self.inner.sender.send(f)
    }

    /// Blocks on the result of the started Proc.
    ///
    /// Do not use if no Proc is being run!
    pub fn join(&self) -> thread::Result<()> {
        self.inner.result.recv().expect("Could not get result")
    }
}

/// The ThreadPool that manages the state of the threads and spawns new ones.
///
/// When requesting a thread, this pool will either return a unused thread that has already been
/// spawned, spin up a new thread, or return an error if the maximum number of threads has been
/// reached.
pub struct ThreadPool {
    free_sender: Sender<Thread>, // Keep this to spawn new threads
    free_threads: Receiver<Thread>, // Threads will send themselves when they are free.
    active_threads: usize,
    max_threads: usize,
}

// TODO: instrument with log crate and log errors
fn spawn_thread(free: Sender<Thread>) {
        spawn(move || {
            let sentinel = Sentinel::new(free.clone());
            let res = ThreadRunner::new(free).run();
            if res.is_ok() { sentinel.done() };
        });
}

impl ThreadPool {
    /// Create a new thread pool with initial and max thread counts.
    pub fn new(init_threads: usize, max_threads: usize) -> ThreadPool {
        let (sn, rc) = channel();
        let mut thrs = Vec::new();
        for _i in 0..init_threads {
            spawn_thread(sn.clone());
            let thr = rc.recv().expect("Could not receive initial thread");
            thrs.push(thr);
        }

        for t in thrs.into_iter() {
            sn.send(t).expect("Could not reinsert threads");
        }

        ThreadPool {
            free_sender: sn,
            free_threads: rc,
            active_threads: init_threads,
            max_threads: max_threads
        }
    }

    /// Returns a handle to a spawned thread or an error if there are no more threads available.
    pub fn thread(&mut self) -> Result<Thread, ThreadError> {
        let mut res = self.free_threads.try_recv();
        if res.is_err() {
            thread::yield_now();
            res = self.free_threads.try_recv();
        }

        match res {
            Ok(thr) => Ok(thr),
            Err(TryRecvError::Empty) => {
                if self.active_threads >= self.max_threads {
                    Err(ThreadError::NoMoreThreads)
                } else {
                    self.active_threads += 1;
                    spawn_thread(self.free_sender.clone());
                    let thr = self.free_threads.recv().expect("Could not receive thread");
                    Ok(thr)
                }
            }
            Err(TryRecvError::Disconnected) => panic!("channel closed"),
        }
    }
}

/// Error type used by the ThreadPool
#[derive(Debug, Clone)]
pub enum ThreadError {
    /// No more threads available
    NoMoreThreads,
    /// An error on sending to a channel
    SendError,
    /// An error on receiving from a channel
    RecvError,
}

impl<T> From<SendError<T>> for ThreadError {
    fn from(_err: SendError<T>) -> ThreadError {
        ThreadError::SendError
    }
}

impl From<RecvError> for ThreadError {
    fn from(_err: RecvError) -> ThreadError {
        ThreadError::RecvError
    }
}

impl Error for ThreadError {
    fn description(&self) -> &str {
        match *self {
            ThreadError::NoMoreThreads => "Could not make any more threads",
            ThreadError::SendError => "Could not send data over channel",
            ThreadError::RecvError => "Could not receive data over channel",
        }
    }
}

impl fmt::Display for ThreadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            ThreadError::NoMoreThreads => "Could not make any more threads.".fmt(f),
            ThreadError::SendError => "Could not send data over channel".fmt(f),
            ThreadError::RecvError => "Could not receive data over channel".fmt(f),
        }
    }
}

struct Sentinel {
    free_threads: Sender<Thread>,
    active: bool,
}

// Idea taken from https://github.com/rust-lang/threadpool/blob/b9416b4cb591a3ac8bac8efef19e5cbf5e212a9d/src/lib.rs#L40
impl Sentinel {
    fn new(free_threads: Sender<Thread>) -> Sentinel {
        Sentinel {
            free_threads: free_threads,
            active: true,
        }
    }

    // Cancel and destroy this sentinel.
    fn done(mut self) {
        self.active = false;
    }
}

impl Drop for Sentinel {
    fn drop(&mut self) {
        if self.active {
            // Spawn new thread and remove old one.
            spawn_thread(self.free_threads.clone());
        }
    }
}

struct ThreadRunner {
    free_chan: Sender<Thread>, // ThreadRunner will send its Thread when it is ready to do work
}

impl ThreadRunner {
    pub fn new(free_chan: Sender<Thread>) -> ThreadRunner {
        ThreadRunner {
            free_chan: free_chan,
        }
    }

    // TODO: How to shut down threads?
    fn run(self) -> Result<(), ThreadError> {
        loop {
            let (thr, jobs) = channel();
            let (res_sender, res_recver) = channel();
            let thread = Thread::new(thr, res_recver);

            try!(self.free_chan.send(thread));
            let job = try!(jobs.recv());
            let res = thread::catch_panic(move || { job.run(); });
            try!(res_sender.send(res));
        }
    }
}

#[cfg(test)]
mod test {
    use std::thread;
    use std::sync::mpsc::{channel};
    use thread_pool::{ThreadPool, Runner};

    #[test]
    fn test_runable() {
        let (sn, rc) = channel::<u8>();
        let f = box move || {
            sn.send(0u8).unwrap();
        };
        f.run();
        assert!(rc.recv().unwrap() == 0);
    }

    #[test]
    fn test_thread_pool_thread_and_start() {
        let (sn, rc) = channel::<u8>();
        let (init, max) = (1, 2);
        let mut pool = ThreadPool::new(init, max);

        let f = box move || {
            sn.send(0u8).unwrap();
        };

        let thr1 = pool.thread();
        let _thr2 = pool.thread();
        let thr3 = pool.thread();
        assert!(thr3.is_err());

        assert!(thr1.is_ok());
        let thr1 = thr1.ok().unwrap();

        let res = thr1.start(f);
        assert!(res.is_ok());

        assert!(rc.recv().unwrap() == 0);
    }

    #[test]
    fn test_thread_pool_join() {
        let (sn1, rc1) = channel::<u8>();
        let (init, max) = (1, 2);
        let mut pool = ThreadPool::new(init, max);

        let f1 = box move || {
            sn1.send(0u8).unwrap();
        };

        let f2 = box move || {
            panic!();
        };

        // Initial thread.
        let thr1 = pool.thread();
        // Start a new one.
        let thr2 = pool.thread();

        assert!(thr1.is_ok());
        let thr1 = thr1.ok().unwrap();

        assert!(thr2.is_ok());
        let thr2 = thr2.ok().unwrap();

        let res = thr1.start(f1);
        assert!(res.is_ok());
        let res = thr2.start(f2);
        assert!(res.is_ok());
        let res = thr1.join();
        assert!(res.is_ok());
        assert!(rc1.recv().unwrap() == 0);

        let res = thr2.join();
        assert!(res.is_err());
    }

    #[test]
    fn test_thread_pool_max_threads_panic() {
        let (sn1, rc1) = channel::<u8>();
        let (sn3, rc3) = channel::<u8>();
        let (init, max) = (1, 2);
        let mut pool = ThreadPool::new(init, max);

        let f1 = box move || {
            sn1.send(0u8).unwrap();
        };

        let f3 = box move || {
            sn3.send(0u8).unwrap();
        };

        let f2 = box move || {
            panic!();
        };

        // Initial thread.
        let thr1 = pool.thread();
        // Start a new one.
        let thr2 = pool.thread();
        let thr1 = thr1.ok().unwrap();
        let thr2 = thr2.ok().unwrap();

        let res = thr2.start(f2);
        assert!(res.is_ok());
        let res = thr2.join();
        assert!(res.is_err());

        thread::sleep_ms(100);

        let thr3 = pool.thread();
        // Can still get thread after panic
        assert!(thr3.is_ok());
        let thr3 = thr3.ok().unwrap();
        let res = thr3.start(f3);
        assert!(res.is_ok());
        let res = thr3.join();
        assert!(res.is_ok());
        assert!(rc3.recv().unwrap() == 0);

        let res = thr1.start(f1);
        assert!(res.is_ok());
        let res = thr1.join();
        assert!(res.is_ok());
        assert!(rc1.recv().unwrap() == 0);
    }
}
