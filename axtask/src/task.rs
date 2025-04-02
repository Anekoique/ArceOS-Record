#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TaskState {
    Running = 1,
    Ready = 2,
    Blocked = 3,
    Exited = 4,
}

struct TaskStack {
    ptr: NonNull<u8>,
    layout: Layout,
}

pub struct Task {
    entry: Option<*mut dyn FnOnce()>,
    state: AtomicU8,
    kstack: Option<TaskStack>,
    ctx: UnsafeCell<TaskContext>,
}

unsafe impl Send for Task {}
unsafe impl Sync for Task {}

pub struct CurrentTask(ManuallyDrop<AxTaskRef>);

pub fn current() -> CurrentTask {
    CurrentTask::get()
}

impl Task {
    pub(crate) fn new_init(name: String) -> AxTaskRef {
        let mut t = Self::new_common(TaskId::new(), name);
        t.is_init = true;
        Arc::new(t)
    }

    fn new_common(id: TaskId, name: String) -> Self {
        Self {
            name,
            entry: None,
            state: AtomicU8::new(TaskState::Ready as u8),
            kstack: None,
            ctx: UnsafeCell::new(TaskContext::new()),
        }
    }

    #[inline]
    pub(crate) fn set_state(&self, state: TaskState) {
        self.state.store(state as u8, Ordering::Release)
    }
}

impl CurrentTask {
    pub(crate) unsafe fn init_current(init_task: AxTaskRef) {
        let ptr = Arc::into_raw(init_task);
        axhal::cpu::set_current_task_ptr(ptr);
    }

    pub(crate) fn try_get() -> Option<Self> {
        let ptr: *const Task = axhal::cpu::current_task_ptr();
        if !ptr.is_null() {
            Some(Self(unsafe { ManuallyDrop::new(AxTaskRef::from_raw(ptr)) }))
        } else {
            None
        }
    }

    pub(crate) fn get() -> Self {
        Self::try_get().expect("current task is uninitialized")
    }
}

pub fn current() -> CurrentTask {
    CurrentTask::get()
}
