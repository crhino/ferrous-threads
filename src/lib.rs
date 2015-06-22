#![feature(core)]
#![feature(alloc)]
#![feature(box_syntax)]
#![feature(scoped)]
#![feature(fnbox)]
#![feature(box_raw)]

mod pool;
pub mod queue;

pub use pool::TaskPool;
pub use queue::{mutex_mpmc_channel, mpmc_channel};
