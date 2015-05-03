/* Christopher Piraino
 *
 *
 * Ferrous Threads
 *
 */
use std::boxed::FnBox;
use std::thread::{scoped, JoinGuard};
use std::sync::mpsc::{SendError, Sender, Receiver, channel};

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

    fn enqueue(&self, task: Task) -> Result<(), SendError<Task>> {
        self.queue.send(task)
    }

    fn worker(rc: Receiver<Task>) {
        loop {
            let msg = rc.recv();
            match msg {
                Ok(task) => task.run(),
                Err(_) => break,
            }
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
        let taskpool = TaskPool::new();

        taskpool.enqueue(task).unwrap();

        assert!(rc1.recv().unwrap() == 10);
    }
}
