mod boot;
pub mod console;
mod lang_items;
mod misc;
mod paging;
pub mod time;
pub use misc::terminate;

unsafe extern "C" fn rust_entry(_hartid: usize, _dtb: usize) {
    unsafe extern "C" {
        fn rust_main(hartid: usize, dtb: usize);
    }
    unsafe {
        rust_main(_hartid, _dtb);
    }
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
