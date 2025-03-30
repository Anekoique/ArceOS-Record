#![no_std]
#![no_main]

mod lang_items;

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
unsafe extern "C" fn _start() -> ! {
    core::arch::asm!(
        "wfi",
        options(noreturn),
    );
}






