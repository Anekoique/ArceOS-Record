use core::time::Duration;
use riscv::register::time;

const TIMER_FREQUENCY: u64 = 10_000_000; // 10MHz
const NANOS_PER_SEC: u64 = 1_000_000_000;
const NANOS_PER_TICK: u64 = NANOS_PER_SEC / TIMER_FREQUENCY;

pub type TimeValue = Duration;

#[inline]
pub fn current_ticks() -> u64 {
    time::read() as u64
}
#[inline]
pub const fn ticks_to_nanos(ticks: u64) -> u64 {
    ticks * NANOS_PER_TICK
}
pub fn current_time_nanos() -> u64 {
    ticks_to_nanos(current_ticks())
}
pub fn current_time() -> TimeValue {
    TimeValue::from_nanos(current_time_nanos())
}
