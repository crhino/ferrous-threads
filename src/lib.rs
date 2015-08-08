#![feature(box_syntax)]
#![feature(fnbox)]
#![feature(box_raw)]
#![feature(drain)]
#![feature(result_expect)]
#![feature(catch_panic)]

#![feature(duration)]
#![feature(time)]
#![feature(thread_sleep)]

//! Ferrous Threads Crate
//!
//! This crate contains a number of different structs and functions that are of use
//! when attempting to do concurrent/parallel programming.
//!
//! This includes a thread pool, a multi-producer/multi-consumer queue, a task runner, and
//! a publish/subscribe queue.

mod thread_pool;
mod task_runner;
mod pubsub;
pub mod queue;

pub use task_runner::TaskRunner;
pub use queue::{mutex_mpmc_channel, mpmc_channel};
pub use pubsub::{pubsub_channel, Subscriber, Publisher};
