/* Christopher Piraino
 *
 *
 * Ferrous Threads
 *
 */
use std::boxed::FnBox;
use std::thread::{self, scoped, JoinGuard};
use queue::{Sender, Receiver, MPMCQueue, mpmc_channel};

pub enum Task {
    Data(TaskData),
    Stop,
}

pub struct TaskData {
    task_func: Box<FnBox() + Send>,
}

impl TaskData {
    pub fn run(self) {
        self.task_func.call_box(())
    }
}

impl Task {
    pub fn new(func: Box<FnBox() + Send>) -> Task {
        Task::Data(TaskData { task_func: func })
    }

    pub fn run(self) {
        match self {
            Task::Data(task) => task.run(),
            Task::Stop => (),
        }
    }
}

struct TaskPool<'a> {
    queue: Sender<Task>,
    workers: Vec<JoinGuard<'a, ()>>,
}

impl<'a> TaskPool<'a> {
    fn new(num_threads: u8) -> TaskPool<'a> {
        let (sn, rc) = mpmc_channel::<Task>(num_threads as usize);
        let mut guards = Vec::new();
        for _i in 0..num_threads {
            let rc = rc.clone();
            let thr = scoped(move || { TaskPool::worker(rc) });
            guards.push(thr);
        }
        TaskPool { queue: sn, workers: guards }
    }

    fn enqueue(&self, task: Task) -> Result<(), Task> {
        self.queue.send(task)
    }

    fn worker(rc: Receiver<Task>) {
        loop {
            let msg = rc.recv();
            match msg {
                Some(Task::Data(task)) => task.run(),
                Some(Task::Stop) => break,
                None  => thread::yield_now(),
            }
        }
    }
}

impl<'a> Drop for TaskPool<'a> {
    fn drop(&mut self) {
        // Send stop message without blocking.
        for _thr in self.workers.iter() {
            self.enqueue(Task::Stop).ok().expect("Could not send Stop message")
        }
    }
}

#[cfg(test)]
mod test {
    use super::{Task, TaskPool};
    use std::sync::mpsc::{channel};

    #[test]
    fn test_task() {
        let (sn, rc) = channel::<u8>();
        let task_closure = move || {
            sn.send(0u8).unwrap();
        };
        let task = Task::new(Box::new(task_closure));
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
        let int_task = Task::new(Box::new(task_closure));

        let task_closure = move || {
            sn2.send(Some(10u8)).unwrap();
        };
        let task = Task::new(Box::new(task_closure));

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
        let task = Task::new(Box::new(task_closure));
        let taskpool = TaskPool::new(1);

        taskpool.enqueue(task).ok().expect("Task not enqueued");
        taskpool.enqueue(Task::Stop).ok().expect("Task not enqueued");

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

        let task1 = Task::new(Box::new(task_closure));
        let task2 = Task::new(Box::new(task_closure2));
        let taskpool = TaskPool::new(3);

        taskpool.enqueue(task1).ok().expect("Task not enqueued");
        taskpool.enqueue(task2).ok().expect("Task not enqueued");
        taskpool.enqueue(Task::Stop).ok().expect("Task not enqueued");

        assert_eq!(rc1.recv().unwrap(), 10);
        assert_eq!(rc1.recv().unwrap(), 10);
    }
}
