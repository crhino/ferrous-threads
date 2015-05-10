trait MPMCQueue<T: Send> {
    fn push(&self, value: T) -> Result<(), T>;
    fn pop(&self) -> Option<T>;
}

trait Sender<T: Send> {
    fn send(&self, value: T) -> Result<(), T>;
}

trait Receiver<T: Send> {
    fn recv(&self) -> Option<T>;
}

// Type passed in must implement MPMCQueue
macro_rules! producer_consumer {
    ($t:ty) => {
        impl<T: Send> Sender<T> for $t {
            fn send(&self, value: T) -> Result<(), T> {
                self.push(value)
            }
        }

        impl<T: Send> Receiver<T> for $t {
            fn recv(&self) -> Option<T> {
                self.pop()
            }
        }
    };
}

mod mutex_linked_list;
mod mpmc_bounded_queue;
