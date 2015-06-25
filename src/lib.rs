#![feature(box_syntax)]
#![feature(fnbox)]
#![feature(box_raw)]
#![feature(drain)]

mod task_runner;
pub mod queue;

pub use task_runner::TaskRunner;
pub use queue::{mutex_mpmc_channel, mpmc_channel};
