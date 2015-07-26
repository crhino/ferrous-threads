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

#[derive(Clone)]
pub struct Thread {
    inner: ThreadInner
}

#[derive(Clone)]
struct ThreadInner {
    sender: Sender<Proc<'static>>,
    result: queue::Receiver<thread::Result<()>>,
}

impl Thread {
    fn new(sender: Sender<Proc<'static>>, result: queue::Receiver<thread::Result<()>>) -> Thread {
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
        self.inner.result.recv().unwrap()
    }
}

pub struct ThreadPool {
    id_sender: Sender<usize>, // Keep this for spawning new threads.
    free_threads: Receiver<usize>, // Threads will send their id when they are free.
    thread_handles: Arc<Mutex<Vec<JoinHandle<()>>>>, // Active threads
    threads: Arc<Mutex<Vec<Thread>>>, // Threads
    max_threads: usize,
}

fn spawn_thread(id: usize, free: Sender<usize>, threads: Arc<Mutex<Vec<Thread>>>, handles: Arc<Mutex<Vec<JoinHandle<()>>>>) -> (JoinHandle<()>, Thread) {
        let (thr, jobs) = channel();
        let (res_sender, res_recver) = mpmc_channel(1);
        let thread = Thread::new(thr, res_recver);

        let handle = spawn(move || {
            let sentinel = Sentinel::new(id, handles, threads);
            ThreadRunner::new(id, res_sender, free, jobs).run();
            sentinel.done();
        });

        (handle, thread)
}

impl ThreadPool {
    pub fn new(init_threads: usize, max_threads: usize) -> ThreadPool {
        let (sn, rc) = channel();
        let threads = Arc::new(Mutex::new(Vec::new()));
        let handles = Arc::new(Mutex::new(Vec::new()));

        {
            let mut locked_thrs = threads.lock().unwrap();
            let mut locked_handles = handles.lock().unwrap();

            for i in 0..init_threads {
                let (handle, thread) = spawn_thread(i, sn.clone(), threads.clone(), handles.clone());
                locked_handles.push(handle);
                locked_thrs.push(thread);
            }
        }

        ThreadPool {
            id_sender: sn,
            free_threads: rc,
            thread_handles: handles,
            threads: threads,
            max_threads: max_threads
        }
    }

    pub fn thread(&self) -> Result<Thread, ThreadError> {
        let thrs = self.threads.lock().unwrap();
        if thrs.len() >= self.max_threads {
            Err(ThreadError)
        } else {
            let res = self.free_threads.try_recv();
            match res {
                Ok(thr_id) => Ok(thrs[thr_id].clone()),
                Err(TryRecvError::Empty) => Err(ThreadError),
                Err(TryRecvError::Disconnected) => panic!("channel closed"),
            }
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
    thread_handles: Arc<Mutex<Vec<JoinHandle<()>>>>, // Active threads
    threads: Arc<Mutex<Vec<Thread>>>, // Thread job queues
    active: bool,
}

// Idea taken from https://github.com/rust-lang/threadpool/blob/b9416b4cb591a3ac8bac8efef19e5cbf5e212a9d/src/lib.rs#L40
impl Sentinel {
    fn new(
        id: usize,
        thread_handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
        threads: Arc<Mutex<Vec<Thread>>>
        ) -> Sentinel {
        Sentinel {
            id: id,
            thread_handles: thread_handles,
            threads: threads,
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
        }
    }
}

struct ThreadRunner {
    id: usize,
    result_chan: queue::Sender<thread::Result<()>>,
    free_chan: Sender<usize>, // ThreadRunner will send its id when it is ready to do work
    jobs: Receiver<Proc<'static>>, // Job queue
}

impl ThreadRunner {
    pub fn new(id: usize, result_chan: queue::Sender<thread::Result<()>>, free_chan: Sender<usize>, jobs: Receiver<Proc<'static>>) -> ThreadRunner {
        ThreadRunner {
            id: id,
            result_chan: result_chan,
            free_chan: free_chan,
            jobs: jobs,
        }
    }

    fn run(self) {
        self.free_chan.send(self.id).unwrap();
        loop {
            let job = self.jobs.recv().unwrap();
            let res = thread::catch_panic(move || { job.run(); });
            self.result_chan.send(res).unwrap();
            self.free_chan.send(self.id).unwrap();
        }
    }
}

#[cfg(test)]
mod test {
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
        let pool = ThreadPool::new(init, max);

        let f = box move || {
            sn.send(0u8).unwrap();
        };

        let thr1 = pool.thread();
        let thr2 = pool.thread();
        let thr3 = pool.thread();
        assert!(thr3.is_err());

        let thr1 = thr1.ok().unwrap();

        let res = thr1.start(f);
        assert!(res.is_ok());

        assert!(rc.recv().unwrap() == 0);
    }

    #[test]
    fn test_thread_pool_join() {
        let (sn1, rc1) = channel::<u8>();
        let (init, max) = (1, 2);
        let pool = ThreadPool::new(init, max);

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
        let thr1 = thr1.ok().unwrap();
        let thr2 = thr2.ok().unwrap();

        thr1.start(f1);
        thr2.start(f2);
        let res = thr1.join();
        assert!(res.is_ok());

        let res = thr2.join();
        assert!(res.is_err());
    }

    #[test]
    fn test_thread_pool_max_threads_panic() {
        let (sn1, rc1) = channel::<u8>();
        let (init, max) = (1, 2);
        let pool = ThreadPool::new(init, max);

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
        let thr1 = thr1.ok().unwrap();
        let thr2 = thr2.ok().unwrap();

        thr2.start(f2);
        let res = thr2.join();
        assert!(res.is_err());

        let thr3 = pool.thread();
        // Can still get thread after panic
        assert!(thr3.is_ok());

        thr1.start(f1);
        let res = thr1.join();
        assert!(res.is_ok());
    }
}
