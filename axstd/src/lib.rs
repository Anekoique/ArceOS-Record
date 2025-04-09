#![no_std]

extern crate alloc;
extern crate axruntime;

pub mod io;
pub mod sync;
pub mod thread;
pub mod time;

// Re-export String
pub use alloc::string::String;
pub use alloc::vec::Vec;
pub use axconfig::*;
pub use axruntime::println;
pub use time::*;
