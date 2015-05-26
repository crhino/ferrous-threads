#![feature(core)]
#![feature(alloc)]
#![feature(box_syntax)]
#![feature(scoped)]

mod pool;
pub mod queue;

pub use pool::TaskPool;
pub use queue::{mutex_mpmc_channel, mpmc_channel};
