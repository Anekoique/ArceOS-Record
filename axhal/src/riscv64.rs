mod boot;
pub mod console;
mod lang_items;
mod paging;

unsafe extern "C" fn rust_entry(_hartid: usize, _dtb: usize) {
    unsafe extern "C" {
        fn rust_main(hartid: usize, dtb: usize);
    }
    unsafe {
        rust_main(_hartid, _dtb);
    }
}
