/* Christopher Piraino
 *
 *
 * Ferrous Threads
 *
 */
#![feature(unboxed_closures)]
#![feature(core)]
use std::boxed::FnBox;
use std::thread::{scoped, JoinGuard};
use std::sync::mpsc::{Sender, Receiver, channel};

pub struct Task {
    task_func: Box<FnBox() + Send>,
}

impl Task {
    pub fn new(func: Box<FnBox() + Send>) -> Task {
        Task { task_func: func }
    }

    pub fn run(self) {
        (self.task_func)()
    }
}

struct TaskPool<'a> {
    queue: Sender<Task>,
    worker: JoinGuard<'a, ()>,
}

impl<'a> TaskPool<'a> {
    fn new() -> TaskPool<'a> {
        let (sn, rc) = channel::<Task>();
        let guard = scoped(move || { TaskPool::worker(rc) });
        TaskPool { queue: sn, worker: guard }
    }

    fn worker(rc: Receiver<Task>) {
        loop {
            let task = rc.recv().unwrap();
            task.run();
        }
    }
}

#[cfg(test)]
mod test {
    use super::Task;
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
}
