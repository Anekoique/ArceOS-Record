mod boot;
pub mod context;
mod lang_items;
mod paging;

pub mod console;
pub mod cpu;
pub mod irq;
pub mod mem;
pub mod misc;
pub mod time;
pub mod trap;

pub use context::TaskContext;
pub use misc::terminate;
pub use paging::write_page_table_root;

unsafe extern "C" fn rust_entry(_hartid: usize, _dtb: usize) {
    unsafe extern "C" {
        fn trap_vector_base();
        fn rust_main(hartid: usize, dtb: usize);
    }
    unsafe {
        trap::set_trap_vector_base(trap_vector_base as usize);
        rust_main(_hartid, _dtb);
    }
}

pub fn platform_init() {
    self::irq::init_percpu();
    self::time::init_percpu();
}

struct LogIfImpl;

#[crate_interface::impl_interface]
impl axlog::LogIf for LogIfImpl {
    fn write_str(s: &str) {
        console::write_bytes(s.as_bytes());
    }

    fn get_time() -> core::time::Duration {
        time::current_time()
    }
}
