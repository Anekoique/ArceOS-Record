use crate::task::*;
extern crate alloc;
use alloc::collections::VecDeque;

pub(crate) static RUN_QUEUE: SpinNoIrq<AxRunQueue> = SpinNoIrq::new(AxRunQueue::new());

pub(crate) struct AxRunQueue {
    ready_queue: VecDeque<Arc<Task>>,
}

impl AxRunQueue {
    pub fn yield_current(&mut self) {
        self.resched();
    }

    fn resched(&mut self) {
        let prev = current();
        if prev.is_running() {
            prev.set_state(TaskState::Ready);
            if !prev.is_idle() {
                self.put_prev_task(prev.clone());
            }
        }
        let next = self.pick_next_task().unwrap(); // FixMe: with IDLE_TASK.get().clone()
        self.switch_to(prev, next);
    }

    fn switch_to(&mut self, prev_task: CurrentTask, next_task: AxTaskRef) {
        next_task.set_preempt_pending(false);
        next_task.set_state(TaskState::Running);
        if prev_task.ptr_eq(&next_task) {
            return;
        }
        todo!("Implement it in future!");
    }

    pub fn pick_next_task(&mut self) -> Option<Arc<Task>> {
        self.ready_queue.pop_front()
    }

    pub fn put_prev_task(&mut self, prev: Arc<Task>, preempt: bool) {
        self.ready_queue.push_back(prev)
    }

    pub fn exit_current(&mut self, exit_code: i32) -> ! {
        axhal::misc::terminate();
    }
}
