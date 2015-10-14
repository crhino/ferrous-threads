/* Christopher Piraino
 *
 *
 * Ferrous Threads
 *
 */
//! A TaskRunner is used when a number of short-lived tasks need to be asynchronously
//! done. Here we negate the cost of thread startup by using a fixed amount of threads
//! to serve a potentially infinite series of tasks.
//!
//! # Warnings
//! Currently the mpmc channel implementation has some bugs, it is currently not recommended
//! to use the TaskRunner yet.
use std::mem;
use std::boxed::FnBox;
use std::fmt;
use std::error::Error;
use std::thread::{spawn, JoinHandle};
use canal::mpmc::{Sender, Receiver, mpmc_channel};

const QUEUE_SIZE: usize = ((0 - 1) as u8) as usize;


enum Task<'a> {
    Data(TaskData<'a>),
    Stop,
}

struct TaskData<'a> {
    task_func: Box<FnBox() + Send + 'a>,
}

impl<'a> TaskData<'a> {
    fn run(self) {
        self.task_func.call_box(())
    }
}

impl<'a> Task<'a> {
    fn new<F>(func: F) -> Task<'a> where F: FnOnce() + Send + 'a {
        Task::Data(TaskData { task_func: box func })
    }
}

/// An error sending to a running thread.
#[derive(Debug)]
pub struct SendError;
impl Error for SendError {
    fn description(&self) -> &str {
        "Could not send to a thread"
    }
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        "Could not send to a thread".fmt(f)
    }
}

/// A TaskRunner is used to run short-lived tasks in parallel without having to
/// spin up a new thread each and every time.
///
/// The TaskRunner will immediately spin up the number of threads that was passed in
/// on creation.
///
/// Spins up a number of threads and distbutes the enqueued tasks through a
/// multi-producer/multi-consumer queue. This allows every worker to draw from the same queue,
/// ensuring that work will be efficiently distributed across the threads.
///
/// # Panics
/// The failure case of a task panicking and destroying the worker is not handled.
///
/// # Examples
/// ```
/// use ferrous_threads::task_runner::TaskRunner;
///
/// use std::sync::mpsc::channel;
///
/// let (sn, rc) = channel();
/// let taskpool = TaskRunner::new(1);
/// taskpool.enqueue(move || { sn.send(9u8).unwrap();}).ok().expect("Task not enqueued");
/// assert!(rc.recv().unwrap() == 9u8);
/// ```
pub struct TaskRunner<'a> {
    queue: Sender<Task<'a>>,
    workers: Vec<JoinHandle<()>>,
}

impl<'a> TaskRunner<'a> {
    /// Create a new TaskRunner with specified number of threads.
    pub fn new(num_threads: u8) -> TaskRunner<'a> {
        let (sn, rc): (Sender<Task<'a>>, Receiver<Task<'a>>) = mpmc_channel::<Task>(QUEUE_SIZE);
        let mut guards = Vec::new();
        for _i in 0..num_threads {
            // spawned threads cannot guarantee lifetimes, but we explicitly join on Drop.
            let rc: Receiver<Task<'static>> = unsafe { mem::transmute(rc.clone()) };
            let thr = spawn(move || { TaskRunner::worker(rc) });
            guards.push(thr);
        }
        TaskRunner { queue: sn, workers: guards }
    }

    /// Places the enqueued function on the worker queue.
    pub fn enqueue<F>(&self, func: F) -> Result<(), SendError> where F: 'a + FnOnce() + Send {
        let task = Task::new(func);
        let res = self.queue.send(task);
        match res {
            Ok(_) => Ok(()),
            Err(_) => Err(SendError),
        }
    }

    fn worker(rc: Receiver<Task>) {
        loop {
            let msg = rc.recv();
            match msg {
                Ok(Task::Data(task)) => task.run(),
                Ok(Task::Stop) => break,
                Err(_)  => break, // TODO: Do something better
            }
        }
    }
}

impl<'a> Drop for TaskRunner<'a> {
    fn drop(&mut self) {
        // Send stop message without blocking.
        for _thr in self.workers.iter() {
            self.queue.send(Task::Stop).ok().expect("Could not send a stop message.");
        }

        for thr in self.workers.drain(..) {
            thr.join().expect("Thread panicked");
        }
    }
}

#[cfg(test)]
mod test {
    use super::{TaskData, TaskRunner};
    use std::sync::mpsc::{channel};

    #[test]
    fn test_task() {
        let (sn, rc) = channel::<u8>();
        let task_closure = move || {
            sn.send(0u8).unwrap();
        };
        let task = TaskData { task_func: box task_closure };
        task.run();
        assert!(rc.recv().unwrap() == 0);
    }

    #[test]
    fn test_task_vector() {
        let (sn1, rc1) = channel::<isize>();
        let (sn2, rc2) = channel::<Option<u8>>();
        let task_closure = move || {
            sn1.send(10).unwrap();
        };
        let int_task = TaskData { task_func: box task_closure };

        let task_closure = move || {
            sn2.send(Some(10u8)).unwrap();
        };
        let task = TaskData { task_func: box task_closure };

        let vec = vec![int_task, task];
        for t in vec.into_iter() {
            t.run();
        }

        assert!(rc1.recv().unwrap() == 10);
        assert!(rc2.recv().unwrap().is_some());
    }

    #[test]
    fn test_task_pool() {
        let (sn1, rc1) = channel::<isize>();
        let task_closure = move || {
            sn1.send(10).unwrap();
        };
        let taskpool = TaskRunner::new(1);

        taskpool.enqueue(task_closure).ok().expect("Task not enqueued");

        assert_eq!(rc1.recv().unwrap(), 10);
    }

    #[test]
    fn test_task_pool_multi_workers() {
        let (sn1, rc1) = channel::<isize>();
        let sn2 = sn1.clone();
        let task_closure = move || {
            sn1.send(10).unwrap();
        };
        let task_closure2 = move || {
            sn2.send(10).unwrap();
        };

        let taskpool = TaskRunner::new(3);

        taskpool.enqueue(task_closure).ok().expect("Task not enqueued");
        taskpool.enqueue(task_closure2).ok().expect("Task not enqueued");

        assert_eq!(rc1.recv().unwrap(), 10);
        assert_eq!(rc1.recv().unwrap(), 10);
    }
}
