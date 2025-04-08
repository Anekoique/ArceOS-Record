#![no_std]

use axconfig::{SIZE_2M, phys_to_virt};
pub use axhal::ax_println as println;
use axhal::mem::{MemRegion, free_regions, kernel_image_regions};
use axsync::BootOnceCell;
use page_table::{PAGE_KERNEL_RW, PageTable};

#[macro_use]
extern crate axlog;

#[unsafe(no_mangle)]
pub extern "C" fn rust_main(hartid: usize, dtb: usize) -> ! {
    unsafe extern "C" {
        fn _skernel();
        fn main();
    }

    println!("\nArceOS is starting ...");
    // We reserve 2M memory range [0x80000000, 0x80200000) for SBI,
    // but it only occupies ~194K. Split this range in half,
    // requisition the higher part(1M) for early heap.
    axalloc::early_init(_skernel as usize - 0x100000, 0x100000);

    axlog::init();
    axlog::set_max_level(option_env!("LOG").unwrap_or(""));
    info!("Logging is enabled.");
    info!("Primary CPU {} started, dtb = {:#x}.", hartid, dtb);
    // Parse fdt for early memory info
    let dtb_info = match parse_dtb(dtb) {
        Ok(info) => info,
        Err(err) => panic!("Bad dtb {:?}", err),
    };

    info!(
        "Memory: {:#x}, size: {:#x}",
        dtb_info.memory_addr, dtb_info.memory_size
    );
    let phys_memory_size = dtb_info.memory_size;
    info!("Virtio_mmio[{}]:", dtb_info.mmio_regions.len());
    for r in &dtb_info.mmio_regions {
        info!("\t{:#x}, size: {:#x}", r.0, r.1);
    }

    info!("Initialize kernel page table...");
    remap_kernel_memory(dtb_info);

    info!("Initialize formal allocators ...");
    for r in free_regions(phys_memory_size) {
        axalloc::final_init(phys_to_virt(r.paddr), r.size);
    }

    axtask::init_scheduler();

    unsafe {
        main();
    }
    axhal::terminate();
}

fn remap_kernel_memory(dtb: DtbInfo) {
    let mmio_regions = dtb.mmio_regions.iter().map(|reg| MemRegion {
        paddr: reg.0,
        size: reg.1,
        flags: PAGE_KERNEL_RW,
        name: "mmio",
    });

    let regions = kernel_image_regions()
        .chain(free_regions(dtb.memory_size))
        .chain(mmio_regions);

    let mut kernel_page_table = PageTable::alloc_table(0);
    for r in regions {
        let _ = kernel_page_table.map(phys_to_virt(r.paddr), r.paddr, r.size, SIZE_2M, r.flags);
    }

    static KERNEL_PAGE_TABLE: BootOnceCell<PageTable> = BootOnceCell::new();
    KERNEL_PAGE_TABLE.init(kernel_page_table);
    unsafe { axhal::write_page_table_root(KERNEL_PAGE_TABLE.get().root_paddr()) };
}

extern crate alloc;

use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use axdtb::SliceRead;
use core::cell::RefCell;
use core::str;

struct DtbInfo {
    memory_addr: usize,
    memory_size: usize,
    mmio_regions: Vec<(usize, usize)>,
}

fn parse_dtb(dtb_pa: usize) -> axdtb::DeviceTreeResult<DtbInfo> {
    let dtb_va = phys_to_virt(dtb_pa);

    // 使用Rc<RefCell<>>包装，允许多所有权和运行时可变借用
    struct TempData {
        memory_addr: usize,
        memory_size: usize,
        mmio_regions: Vec<(usize, usize)>,
    }

    let temp_data = Rc::new(RefCell::new(TempData {
        memory_addr: 0,
        memory_size: 0,
        mmio_regions: Vec::new(),
    }));

    // 创建适配器闭包
    let temp_data_clone = temp_data.clone();
    let mut cb = move |_name: String,
                       addr_cells: usize,
                       size_cells: usize,
                       props: Vec<axdtb::DeviceTreeProperty>| {
        let mut is_memory = false;
        let mut is_mmio = false;
        let mut reg = None;

        for prop in props {
            match prop.0.as_str() {
                "device_type" => {
                    is_memory =
                        str::from_utf8(&(prop.1)).map_or_else(|_| false, |v| v == "memory\0");
                }
                "compatible" => {
                    is_mmio =
                        str::from_utf8(&(prop.1)).map_or_else(|_| false, |v| v == "virtio,mmio\0");
                }
                "reg" => {
                    reg = Some(prop.1);
                }
                _ => (),
            }
        }

        let mut data = temp_data_clone.borrow_mut();
        if is_memory {
            assert!(addr_cells == 2);
            assert!(size_cells == 2);
            if let Some(ref reg) = reg {
                data.memory_addr = reg.as_slice().read_be_u64(0).unwrap() as usize;
                data.memory_size = reg.as_slice().read_be_u64(8).unwrap() as usize;
            }
        }
        if is_mmio {
            assert!(addr_cells == 2);
            assert!(size_cells == 2);
            if let Some(ref reg) = reg {
                let addr = reg.as_slice().read_be_u64(0).unwrap() as usize;
                let size = reg.as_slice().read_be_u64(8).unwrap() as usize;
                data.mmio_regions.push((addr, size));
            }
        }
    };

    let dt = axdtb::DeviceTree::init(dtb_va)?;
    dt.parse(dt.off_struct, 0, 0, &mut cb)?;

    // 从Rc<RefCell<>>中提取结果
    let data = temp_data.borrow();
    Ok(DtbInfo {
        memory_addr: data.memory_addr,
        memory_size: data.memory_size,
        mmio_regions: data.mmio_regions.clone(),
    })
}
