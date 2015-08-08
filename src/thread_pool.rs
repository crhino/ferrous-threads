use std::boxed::FnBox;
use std::thread::{self, spawn, JoinHandle};
use std::sync::mpsc::{channel, Sender, SendError, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::error::Error;
use std::fmt;

use queue;
use mpmc_channel;

trait Runner {
    fn run(self: Box<Self>);
}

impl <F: FnBox()> Runner for F {
    fn run(self: Box<F>) {
        self.call_box(())
    }
}

type Proc<'a> = Box<Runner + Send + 'a>;

pub struct Thread {
    inner: ThreadInner
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

    pub fn start(&self, f: Proc<'static>) -> Result<(), SendError<Proc<'static>>> {
        self.inner.sender.send(f)
    }

    pub fn join(&self) -> thread::Result<()> {
        self.inner.result.recv().expect("Could not get result")
    }
}

pub struct ThreadPool {
    free_sender: Sender<Thread>, // Keep this to spawn new threads
    free_threads: Receiver<Thread>, // Threads will send themselves when they are free.
    active_threads: usize,
    max_threads: usize,
}

fn spawn_thread(id: usize, free: Sender<Thread>) {
        spawn(move || {
            let sentinel = Sentinel::new(id, free.clone());
            ThreadRunner::new(id, free).run();
            sentinel.done();
        });
}

impl ThreadPool {
    pub fn new(init_threads: usize, max_threads: usize) -> ThreadPool {
        let (sn, rc) = channel();
        let mut thrs = Vec::new();
        for i in 0..init_threads {
            spawn_thread(i, sn.clone());
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

    pub fn thread(&mut self) -> Result<Thread, ThreadError> {
        let mut res = Err(TryRecvError::Disconnected);
        for _i in 0..9 {
            res = self.free_threads.try_recv();
            if res.is_ok() {
                break
            }
        }

        match res {
            Ok(thr) => Ok(thr),
            Err(TryRecvError::Empty) => {
                if self.active_threads >= self.max_threads {
                    Err(ThreadError)
                } else {
                    self.active_threads += 1;
                    spawn_thread(self.active_threads, self.free_sender.clone());
                    let thr = self.free_threads.recv().expect("Could not receive thread");
                    Ok(thr)
                }
            }
            Err(TryRecvError::Disconnected) => panic!("channel closed"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThreadError;

impl Error for ThreadError {
    fn description(&self) -> &str {
        "Could not make any more threads."
    }
}

impl fmt::Display for ThreadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        "Could not make any more threads.".fmt(f)
    }
}

struct Sentinel {
    id: usize,
    free_threads: Sender<Thread>,
    active: bool,
}

// Idea taken from https://github.com/rust-lang/threadpool/blob/b9416b4cb591a3ac8bac8efef19e5cbf5e212a9d/src/lib.rs#L40
impl Sentinel {
    fn new(id: usize, free_threads: Sender<Thread>) -> Sentinel {
        Sentinel {
            id: id,
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
            spawn_thread(self.id, self.free_threads.clone());
        }
    }
}

struct ThreadRunner {
    id: usize,
    free_chan: Sender<Thread>, // ThreadRunner will send its Thread when it is ready to do work
}

impl ThreadRunner {
    pub fn new(id: usize, free_chan: Sender<Thread>) -> ThreadRunner {
        ThreadRunner {
            id: id,
            free_chan: free_chan,
        }
    }

    fn run(self) {
        loop {
            let (thr, jobs) = channel();
            let (res_sender, res_recver) = channel();
            let thread = Thread::new(thr, res_recver);

            let res = self.free_chan.send(thread);
            if res.is_err() {
                // ThreadPool has disconnected
                return
            }
            let res = jobs.recv();
            if res.is_err() {
                // Thread has disconnected
                return
            }
            let job = res.unwrap();
            let res = thread::catch_panic(move || { job.run(); });
            res_sender.send(res).expect("Could not send result");
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::mpsc::{channel};
    use std::thread;
    use std::time;
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
        let thr2 = pool.thread();
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
        thr2.start(f2);
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

        thr2.start(f2);
        let res = thr2.join();
        assert!(res.is_err());

        let thr3 = pool.thread();
        // Can still get thread after panic
        assert!(thr3.is_ok());
        let thr3 = thr3.ok().unwrap();
        thr3.start(f3);
        let res = thr3.join();
        assert!(res.is_ok());
        assert!(rc3.recv().unwrap() == 0);

        thr1.start(f1);
        let res = thr1.join();
        assert!(res.is_ok());
        assert!(rc1.recv().unwrap() == 0);
    }
}
