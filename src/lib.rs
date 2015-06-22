#![feature(box_syntax)]
#![feature(fnbox)]
#![feature(box_raw)]
#![feature(drain)]

mod pool;
pub mod queue;

pub use pool::TaskPool;
pub use queue::{mutex_mpmc_channel, mpmc_channel};
