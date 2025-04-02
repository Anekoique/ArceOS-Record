#![no_std]

mod run_queue;
pub mod task;

pub fn yield_now() {
    run_queue::RUN_QUEUE.lock().yield_current();
}
