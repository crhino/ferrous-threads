#![feature(box_syntax)]
#![feature(fnbox)]
#![feature(box_raw)]
#![feature(drain)]
#![feature(result_expect)]
#![feature(catch_panic)]

mod thread_pool;
mod task_runner;
mod pubsub;
pub mod queue;

pub use task_runner::TaskRunner;
pub use queue::{mutex_mpmc_channel, mpmc_channel};
