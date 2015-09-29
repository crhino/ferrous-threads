#![feature(box_syntax)]
#![feature(fnbox)]
#![feature(drain)]
#![feature(result_expect)]
#![feature(catch_panic)]

//! Ferrous Threads Crate
//!
//! This crate contains a number of different structs and functions that are of use
//! when attempting to do concurrent/parallel programming.
//!
//! This includes a thread pool, a multi-producer/multi-consumer queue, a task runner, and
//! a publish/subscribe queue.

extern crate canal;

pub mod task_runner;
pub mod thread_pool;
