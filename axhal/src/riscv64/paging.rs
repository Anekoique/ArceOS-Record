use axconfig::{SIZE_1G, phys_pfn};
use page_table::{PAGE_KERNEL_RWX, PageTable};
use riscv::register::satp;

unsafe extern "C" {
    unsafe fn boot_page_table();
}

pub unsafe fn init_boot_page_table() {
    let mut pt: PageTable = PageTable::init(boot_page_table as usize, 0);
    let _ = pt.map(0x8000_0000, 0x8000_0000, SIZE_1G, SIZE_1G, PAGE_KERNEL_RWX);
    let _ = pt.map(
        0xffff_ffc0_8000_0000,
        0x8000_0000,
        SIZE_1G,
        SIZE_1G,
        PAGE_KERNEL_RWX,
    );
}

pub unsafe fn init_mmu() {
    unsafe {
        write_page_table_root(boot_page_table as usize);
    }
}

/// Writes the physical address of the page table root to the SATP register.
///
/// # Safety
///
/// The caller must ensure that `pa` points to a valid page table structure
/// that is properly initialized and aligned. Incorrect page tables can cause
/// memory access violations and system crashes.
pub unsafe fn write_page_table_root(pa: usize) {
    unsafe {
        satp::set(satp::Mode::Sv39, 0, phys_pfn(pa));
        riscv::asm::sfence_vma_all();
    }
}
