#![feature(core)]
#![feature(alloc)]
#![feature(box_syntax)]
#![feature(scoped)]

mod pool;
pub mod queue;

pub use pool::TaskPool;
