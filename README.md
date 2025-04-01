# ArceOS from scratch

## Ch0 Hello World

Target：让qemu成功跳转到内核入口，执行指令

```rust
// ch0 code framework
.
├── axorigin
│   ├── Cargo.lock
│   ├── Cargo.toml
│   └── src
│       ├── lang_items.rs
│       └── main.rs
├── linker.lds
├── Makefile
├── qemu.log
├── README.md
└── rust-toolchain.toml
```

### 一、启动内核

qemu-riscv启动内核过程

1. PC初始化为0x1000

2. qemu在0x1000预先放置ROM，执行其中代码，跳转到0x8000_0000
3. qemu在0x8000_0000预先放置OpenSBI，由此启动SBI，进行硬件初始化工作，提供功能调用；M-Mode切换到S-Mode，跳转到0x8020_0000
4. 我们将内核加载到0x8020_0000,启动内核

> [!CAUTION]
>
> riscv 体系结构及平台设计的简洁性：
>
> RISC-V SBI 规范定义了平台固件应当具备的功能和服务接口，多数情况下 SBI 本身就可以代替传统上固件 BIOS/UEFI + BootLoader 的位置和作用
>
> // qemu-riscv 启动
> QEMU 启动 → 加载内置 OpenSBI（固件） → 直接跳转至内核入口 → 内核运行
> // qemu-x86 启动
> QEMU 启动 → 加载 SeaBIOS（传统 BIOS） → 加载 GRUB（BootLoader） → 解析配置文件 → 加载内核 → 内核运行

### 二、建立内核程序入口

```toml
// 准备编译工具链
// rust-roolchain.toml
[toolchain]
profile = "minimal"
channel = "nightly"
components = ["rust-src", "llvm-tools-preview", "rustfmt", "clippy"]
targets = ["riscv64gc-unknown-none-elf"]
```

```asm
// 将内核入口放置在image文件开头
// linker.lds
OUTPUT_ARCH(riscv)

BASE_ADDRESS = 0x80200000;

ENTRY(_start)
SECTIONS
{
    . = BASE_ADDRESS;
    _skernel = .;

    .text : ALIGN(4K) {
        _stext = .;
        // 内核入口标记：*(.text.boot)
        *(.text.boot)                      
        *(.text .text.*)
        . = ALIGN(4K);
        _etext = .;
    }
    ...
}
```

```rust
// axorigin/src/main.rs
#![no_std]
#![no_main]
mod lang_items;                                         // 需要实现panic handler处理不可恢复异常

#[unsafe(no_mangle)]                                    // 要求编译器保持_start函数名称
#[unsafe(link_section = ".text.boot")]                  // 将_start放置在链接文件标记处
unsafe extern "C" fn _start() -> ! {
    unsafe {
        core::arch::asm!("wfi", options(noreturn),);    // 嵌入一条汇编
    }
}

// axorigin/src/lang_items.rs
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

```shell
# 编译axorigin
cargo build --manifest-path axorigin/Cargo.toml \
    --target riscv64gc-unknown-none-elf --target-dir ./target --release
# 将编译得到的ELF文件转为二进制格式
rust-objcopy --binary-architecture=riscv64 --strip-all -O binary \
    target/riscv64gc-unknown-none-elf/release/axorigin \
    target/riscv64gc-unknown-none-elf/release/axorigin.bin
# 运行qemu
qemu-system-riscv64 -m 128M -machine virt -bios default -nographic \
    -kernel target/riscv64gc-unknown-none-elf/release/axorigin.bin \
    -D qemu.log -d in_asm
```

---

## Ch1 Hello ArceOS

Target：以组件化方式解耦Unikernel内核，实现输出

```rust
// ch1 code framework
.
├── axconfig
│   ├── Cargo.toml
│   ├── src
│   │   └── lib.rs
│   └── tests
│       └── test_align.rs
├── axhal
│   ├── Cargo.lock
│   ├── Cargo.toml
│   ├── linker.lds
│   └── src
│       ├── lib.rs
│       ├── riscv64
│       │   ├── boot.rs
│       │   ├── console.rs
│       │   └── lang_items.rs
│       └── riscv64.rs
├── axorigin
│   ├── Cargo.lock
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── axsync
│   ├── Cargo.toml
│   ├── src
│   │   ├── bootcell.rs
│   │   └── lib.rs
│   └── tests
│       └── test_bootcell.rs
├── Cargo.lock
├── Cargo.toml
├── Makefile
├── qemu.log
├── README.md
├── rust-toolchain.toml
└── spinlock
    ├── Cargo.toml
    ├── src
    │   ├── lib.rs
    │   └── raw.rs
    └── tests
        └── test_raw.rs
```

### 一、Unikernel与组件化

Unikernel：单内核，将应用程序与kernel编译为单一image

Monolithic：内核态，用户态（Linux

Microkernel：仅在内核保留最基础功能，其他服务在用户态

组件化：组件作为模块封装功能，提供接口，各个组件构成操作系统的基本元素；以构建 crate 的方式来构建组件，通过 dependencies+features 的方式组合组件->` ArceOS = 组件仓库 + 组合方式 `

### 二、解耦Unikernel

1. 根据功能将Unikernel分为系统层(axhal)和应用层(axorigin)

系统层：封装对体系结构和具体硬件的支持和操作，对外提供硬件操作接口

应用层：实现具体功能，在Ch1中为调用axhal提供的console功能，输出信息

```rust
// 系统层实现内核启动：
// 1. 清零BSS区域
// 2. 建立栈支持函数调用
// 3. 跳转到rust_entry

// axhal/src/boot.rs
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
unsafe extern "C" fn _start() -> ! {
    // a0 = hartid
    // a1 = dtb
    core::arch::asm!("
        la a3, _sbss
        la a4, _ebss
        ble a4, a3, 2f
1:
        sd zero, (a3)
        add a3, a3, 8
        blt a3, a4, 1b
2:

        la      sp, boot_stack_top      // setup boot stack（定义在linker.lds

        la      a2, {entry}
        jalr    a2                      // call rust_entry(hartid, dtb)
        j       .",
        entry = sym super::rust_entry,
        options(noreturn),
    )
}

// 通过axhal的rust_entry跳转到axorigin的main入口
// axhal/src/lib.rs
#![no_std]
#![no_main]

mod lang_items;
mod boot;
pub mod console;

unsafe extern "C" fn rust_entry(_hartid: usize, _dtb: usize) {
    unsafe extern "C" {
        fn main(hartid: usize, dtb: usize);
    }
    main(_hartid, _dtb);
}
```

```rust
// axhal使用SBI提供的功能调用，实现字符输出
// axhal与SBI交互的过程也体现了其作为系统层封装硬件操作，对外提供硬件操作接口的功能
// axhal/src/console.rs
use core::fmt::{Error, Write};

struct Console;

pub fn putchar(c: u8) {
    #[allow(deprecated)]
    sbi_rt::legacy::console_putchar(c as usize);
}

pub fn write_bytes(bytes: &[u8]) {
    for c in bytes {
        putchar(*c);
    }
}

impl Write for Console {
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        write_bytes(s.as_bytes());
        Ok(())
    }
}

pub fn __print_impl(args: core::fmt::Arguments) {
    Console.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! ax_print {
    ($($arg:tt)*) => {
        $crate::console::__print_impl(format_args!($($arg)*));
    }
}

#[macro_export]
macro_rules! ax_println {
    () => { $crate::print!("\n") };
    ($($arg:tt)*) => {
        $crate::console::__print_impl(format_args!("{}\n", format_args!($($arg)*)));
    }
}
```

2. 将两个组件进行组合，形成可运行Unikernel

```toml
// axhal/Cargo.toml
[package]
name = "axhal"
version = "0.1.0"
edition = "2021"

[dependencies]
sbi-rt = { version = "0.0.2", features = ["legacy"] }

// axorigin/Cargo.toml  
[package]
name = "axorigin"
version = "0.1.0"
edition = "2021"

[dependencies]
axhal = { path = "../axhal" }

```

### 三、组件测试/模块测试

组件测试相当于 crate 级的测试，直接在 crate 根目录下建立 tests 模块，对组件进行基于公开接口的黑盒测试；模块测试对应于单元测试，在模块内建立子模块tests，对模块的内部方法进行白盒测试

> [!CAUTION]
>
> make run 和 make test编译执行环境不同，make run编译目标为riscv架构，运行在qumu上，make test相当于将测试作为应用直接运行于x86 linux(your pc)上

```makefile
# 互相隔离的环境，test下不能有RUSTFLAGS环境变量
ifeq ($(filter $(MAKECMDGOALS),test),)  # not run `cargo test`
RUSTFLAGS := -C link-arg=-T$(LD_SCRIPT) -C link-arg=-no-pie
export RUSTFLAGS
endif

test:
	cargo test --workspace --exclude "axorigin" -- --nocapture
	
# 为了便于测试，增加测试模块功能：
ifeq ($(filter test test_mod,$(MAKECMDGOALS)),)  # not run `cargo test` 或 `cargo test_mod`
RUSTFLAGS := -C link-arg=-T$(LD_SCRIPT) -C link-arg=-no-pie
export RUSTFLAGS
endif

test_mod:
ifndef MOD
		@printf "    $(YELLOW_C)Error$(END_C): Please specify a module using MOD=<module_name>\n"
		@printf "    Example: make test_mod MOD=axhal\n"
else
		@printf "    $(GREEN_C)Testing$(END_C) module: $(MOD)\n"
		cargo test --package $(MOD) -- --nocapture
endif
```

```toml
# 支持test，工程 Workspace 级的 Cargo.toml
// ArceOS/Cargo.toml
[workspace]
resolver = "2"

members = [
    "axorigin",
    "axhal",
]

[profile.release]
lto = true
```

```rust
// axhal需要屏蔽体系结构差异，原来在riscv64环境下的实现代码应该被隔离
// axhal/src/lib.rs
#![no_std]

#[cfg(target_arch = "riscv64")]
mod riscv64;
#[cfg(target_arch = "riscv64")]
pub use self::riscv64::*;

// axhal/src/riscv64.rs
mod lang_items;
mod boot;
pub mod console;

unsafe extern "C" fn rust_entry(_hartid: usize, _dtb: usize) {
    unsafe extern "C" {
        fn main(hartid: usize, dtb: usize);
    }
    main(_hartid, _dtb);
}

// axhal/Cargo.toml
[target.'cfg(target_arch = "riscv64")'.dependencies]
sbi-rt = { version = "0.0.2", features = ["legacy"] }
```

`axhal`隔离riscv64环境后：

```shell
 Cargo.toml                            | 10 ++++++++++
 Makefile                              |  7 ++++++-
 axhal/Cargo.toml                      |  2 +-
 axhal/src/lib.rs                      | 15 ++++-----------
 axhal/src/riscv64.rs                  | 10 ++++++++++
 axhal/src/{ => riscv64}/boot.rs       |  0
 axhal/src/{ => riscv64}/console.rs    |  0
 axhal/src/{ => riscv64}/lang_items.rs |  0
 8 files changed, 31 insertions(+), 13 deletions(-)
```

### 四、添加组件

1. 全局配置组件 `axconfig`

   ```rust
   // 全局常量 与 工具函数
   // axconfig/src/lib.rs
   #![no_std]
   
   pub const PAGE_SHIFT: usize = 12;
   pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;
   pub const PHYS_VIRT_OFFSET: usize = 0xffff_ffc0_0000_0000;
   pub const ASPACE_BITS: usize = 39;
   
   pub const SIZE_1G: usize = 0x4000_0000;
   pub const SIZE_2M: usize = 0x20_0000;
   
   #[inline]
   pub const fn align_up(val: usize, align: usize) -> usize {
       (val + align - 1) & !(align - 1)
   }
   #[inline]
   pub const fn align_down(val: usize, align: usize) -> usize {
       (val) & !(align - 1)
   }
   #[inline]
   pub const fn align_offset(addr: usize, align: usize) -> usize {
       addr & (align - 1)
   }
   #[inline]
   pub const fn is_aligned(addr: usize, align: usize) -> bool {
       align_offset(addr, align) == 0
   }
   #[inline]
   pub const fn phys_pfn(pa: usize) -> usize {
       pa >> PAGE_SHIFT
   }
   ```

2. 自旋锁 `SpinRaw`

   ```rust
   // 并没有真正实现自旋锁，目前处于单线程状态不会出现数据争用问题
   // 自旋锁的作用是为了包装mut全局变量，假装实现同步保护
   // spinlock/src/lib.rs
   #![no_std]
   mod raw;
   pub use raw::{SpinRaw, SpinRawGuard}
   
   // spinlock/src/raw.rs
   use core::cell::UnsafeCell;
   use core::ops::{Deref, DerefMut};
   
   pub struct SpinRaw<T> {
       data: UnsafeCell<T>,
   }
   
   pub struct SpinRawGuard<T> {
       data: *mut T,
   }
   
   unsafe impl<T> Sync for SpinRaw<T> {}
   unsafe impl<T> Send for SpinRaw<T> {}
   
   impl<T> SpinRaw<T> {
       #[inline(always)]
       pub const fn new(data: T) -> Self {
           Self {
               data: UnsafeCell::new(data),
           }
       }
   
       #[inline(always)]
       pub fn lock(&self) -> SpinRawGuard<T> {
           SpinRawGuard {
               data: unsafe { &mut *self.data.get() },
           }
       }
   }
   
   impl<T> Deref for SpinRawGuard<T> {
       type Target = T;
       #[inline(always)]
       fn deref(&self) -> &T {
           unsafe { &*self.data }
       }
   }
   
   impl<T> DerefMut for SpinRawGuard<T> {
       #[inline(always)]
       fn deref_mut(&mut self) -> &mut T {
           unsafe { &mut *self.data }
       }
   }
   ```

   > [!NOTE]
   >
   > UnsafeCell<T>是实现 **内部可变性（Interior Mutability）** 的核心机制
   >
   > - **常规规则**：
   >   Rust 默认通过引用规则禁止通过不可变引用（`&T`）修改数据，只能通过可变引用（`&mut T`）修改，且同一作用域内只能存在一个可变引用。
   >
   >   ```rust
   >   let x = 42;
   >   let y = &x;
   >   *y = 10; // 错误：不能通过不可变引用修改数据
   >   ```
   >
   > - **`UnsafeCell` 的魔法**：
   >   通过 `UnsafeCell<T>` 包裹数据，允许在不可变引用（`&UnsafeCell<T>`）的上下文中修改内部数据：
   >
   >   ```rust
   >   use core::cell::UnsafeCell;
   >         
   >   let cell = UnsafeCell::new(42);
   >   let ptr = cell.get(); // 获取 *mut T 裸指针
   >   unsafe { *ptr = 10; } // 允许修改
   >   ```

3. 初始化组件`axsync`

   ```rust
   // BootOnceCell 是对 lazy_static 的替代实现
   // 初始化阶段单线程：仅在启动阶段由单个线程调用 init()
   // 初始化后只读：初始化完成后，所有线程仅通过 get() 读取数据
   // axsync/src/lib.rs
   #![no_std]
   
   mod bootcell;
   pub use bootcell::BootOnceCell;
   
   // axsync/src/bootcell.rs
   use core::cell::OnceCell;
   
   pub struct BootOnceCell<T> {
       inner: OnceCell<T>,
   }
   
   impl<T> BootOnceCell<T> {
       pub const fn new() -> Self {
           Self {
               inner: OnceCell::new()
           }
       }
   
       pub fn init(&self, val: T) {
           let _ = self.inner.set(val);
       }
   
       pub fn get(&self) -> &T {
           self.inner.get().unwrap()
       }
   
       pub fn is_init(&self) -> bool {
           self.inner.get().is_some()
       }
   }
   
   unsafe impl<T> Sync for BootOnceCell<T> {}
   ```
   
---

## Ch2 内存管理

Target：组织Unikernel为四层系统架构，引入虚拟内存和内存分配

```shell
# Ch2 Code Framework
.
├── axalloc
│   ├── Cargo.toml
│   └── src
│       ├── early
│       │   └── tests.rs
│       ├── early.rs
│       └── lib.rs
├── axconfig
│   ├── Cargo.toml
│   ├── src
│   │   └── lib.rs
│   └── tests
│       └── test_align.rs
├── axhal
│   ├── Cargo.lock
│   ├── Cargo.toml
│   ├── linker.lds
│   └── src
│       ├── lib.rs
│       ├── riscv64
│       │   ├── boot.rs
│       │   ├── console.rs
│       │   ├── lang_items.rs
│       │   └── paging.rs
│       └── riscv64.rs
├── axorigin
│   ├── Cargo.lock
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── axruntime
│   ├── Cargo.toml
│   └── src
│       └── lib.rs
├── axstd
│   ├── Cargo.toml
│   └── src
│       └── lib.rs
├── axsync
│   ├── Cargo.toml
│   ├── src
│   │   ├── bootcell.rs
│   │   └── lib.rs
│   └── tests
│       └── test_bootcell.rs
├── Cargo.lock
├── Cargo.toml
├── Makefile
├── page_table
│   ├── Cargo.toml
│   ├── src
│   │   └── lib.rs
│   └── tests
│       └── test_ealy.rs
├── qemu.log
├── README.md
├── rust-toolchain.toml
└── spinlock
    ├── Cargo.toml
    ├── src
    │   ├── lib.rs
    │   └── raw.rs
    └── tests
        └── test_raw.rs
```

### 一、内核框架构建

原来的系统只有系统层axhal和应用层axorigin，我们需要另外增加一层管理组织组件的axstd层，它负责了应用和系统的隔离，抽象了系统为应用提供的功能；另外axhal负责了与硬件体系相关的工作，另外一些组件的初始化（硬件体系无关）需要一个位于axhal之上的抽象层来负责，因此引入了axruntime提供运行时管理

```rust
// 启动过程axhal -> axruntime -> axorigin (axstd作为axorigin的依赖载入)
// axhal/src/riscv64.rs
mod lang_items;
mod boot;
pub mod console;
mod paging;

unsafe extern "C" fn rust_entry(hartid: usize, dtb: usize) {
    extern "C" {
        fn rust_main(hartid: usize, dtb: usize);
    }
    rust_main(hartid, dtb);
}

// axruntime/src/lib.rs
#![no_std]

pub use axhal::ax_println as println;

#[no_mangle]
pub extern "C" fn rust_main(_hartid: usize, _dtb: usize) -> ! {
    extern "C" {
        fn _skernel();
        fn main();
    }

    println!("\nArceOS is starting ...");
    // We reserve 2M memory range [0x80000000, 0x80200000) for SBI,
    // but it only occupies ~194K. Split this range in half,
    // requisition the higher part(1M) for early heap.
    axalloc::early_init(_skernel as usize - 0x100000, 0x100000);    // 加载组件启用内存分配器
    unsafe { main(); }
    loop {}
}

// axorigin/src/main.rs
#![no_std]
#![no_main]

use axstd::{String, println};

#[no_mangle]
pub fn main(_hartid: usize, _dtb: usize) {
    let s = String::from("from String");
    println!("\nHello, ArceOS![{}]", s);
}
```

### 二、引入分页机制

### 三、启用动态内存分配

在第一部分的框架代码中我们使用了String，但是String的实现需要动态内存分配支持，这本来是rust std库提供的功能，我们需要实现内核级的动态内存堆管理的支持，因此我们引入了axalloc组件，它的初始化在axruntime中完成。

全局内存分配器 GlobalAllocator 实现 GlobalAlloc Trait，它包含两个功能：字节分配和页分配，分别用于响应对应请求。区分两种请求的策略是，请求分配的大小是页大小的倍数且按页对齐，就视作申请页；否则就是按字节申请分配。我们在GlobalAllocator的下层实现一个early allocator提供底层内存分配的抽象封装

```rust
// axalloc/src/lib.rs
#![no_std]
use axconfig::PAGE_SIZE;
use core::alloc::Layout;
use core::ptr::NonNull;
use spinlock::SpinRaw;

extern crate alloc;
use alloc::alloc::GlobalAlloc;

mod early;
use early::EarlyAllocator;

#[derive(Debug)]
pub enum AllocError {
    InvalidParam,
    MemoryOverlap,
    NoMemory,
    NotAllocated,
}
pub type AllocResult<T = ()> = Result<T, AllocError>;

// 通过 #[global_allocator] 属性将 GlobalAllocator 注册为全局分配器，使 String 等类型默认使用它
#[cfg_attr(not(test), global_allocator)]
static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();

struct GlobalAllocator {
    // 使用实现的自旋锁组件，防止争用
    early_alloc: SpinRaw<EarlyAllocator>,
}

impl GlobalAllocator {
    pub const fn new() -> Self {
        Self {
            early_alloc: SpinRaw::new(EarlyAllocator::uninit_new()),
        }
    }

    pub fn early_init(&self, start: usize, size: usize) {
        self.early_alloc.lock().init(start, size)
    }
}

impl GlobalAllocator {
    fn alloc_bytes(&self, layout: Layout) -> *mut u8 {
        if let Ok(ptr) = self.early_alloc.lock().alloc_bytes(layout) {
            ptr.as_ptr()
        } else {
            alloc::alloc::handle_alloc_error(layout)
        }
    }
    fn dealloc_bytes(&self, ptr: *mut u8, layout: Layout) {
        self.early_alloc
            .lock()
            .dealloc_bytes(NonNull::new(ptr).expect("dealloc null ptr"), layout)
    }
    fn alloc_pages(&self, layout: Layout) -> *mut u8 {
        if let Ok(ptr) = self.early_alloc.lock().alloc_pages(layout) {
            ptr.as_ptr()
        } else {
            alloc::alloc::handle_alloc_error(layout)
        }
    }
    fn dealloc_pages(&self, _ptr: *mut u8, _layout: Layout) {
        unimplemented!();
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() % PAGE_SIZE == 0 && layout.align() == PAGE_SIZE {
            self.alloc_pages(layout)
        } else {
            self.alloc_bytes(layout)
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if layout.size() % PAGE_SIZE == 0 && layout.align() == PAGE_SIZE {
            self.dealloc_pages(ptr, layout)
        } else {
            self.dealloc_bytes(ptr, layout)
        }
    }
}

pub fn early_init(start: usize, len: usize) {
    GLOBAL_ALLOCATOR.early_init(start, len)
}
```

底层内存分配器

> [!TIP]
>
> `Layout` 是 Rust 标准库中定义的一个结构体，用于描述内存分配的 **布局要求**，包含两个核心字段：
>
> - **`size: usize`**：需要分配的内存块大小。
> - **`align: usize`**：内存块的最小对齐要求
>
> ``` 
> // 分配一个 16 字节、按 8 字节对齐的内存块
> let layout = Layout::from_size_align(16, 8).unwrap();
> ```
>
> ##### `byte_pos` 和 `page_pos` 的意义
>
> `EarlyAllocator` 通过两个指针管理内存区域：
>
> - **`byte_pos`**：从 **低地址向高地址** 分配小粒度内存（字节级）。
> - **`page_pos`**：从 **高地址向低地址** 分配大粒度内存（页级）
>
> **设计特点**：
>
> - 仅在所有字节分配都被释放时（`count == 0`），才将 `byte_pos` 重置到 `start`。
> - 不支持部分释放，适用于 **批量分配后一次性释放** 的场景（如临时缓冲区）。
> - page_dealloc为支持

```rust
// axalloc/src/early.rs
#![allow(dead_code)]

#[cfg(test)]
mod tests;

use crate::{AllocError, AllocResult};
use axconfig::{PAGE_SIZE, align_down, align_up};
use core::alloc::Layout;
use core::ptr::NonNull;

#[derive(Default)]
pub struct EarlyAllocator {
    start: usize,
    end: usize,
    count: usize,
    byte_pos: usize,
    page_pos: usize,
}

impl EarlyAllocator {
    pub fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.end = start + size;
        self.byte_pos = start;
        self.page_pos = self.end;
    }
    pub const fn uninit_new() -> Self {
        Self {
            start: 0,
            end: 0,
            count: 0,
            byte_pos: 0,
            page_pos: 0,
        }
    }
}

impl EarlyAllocator {
    pub fn alloc_pages(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        assert_eq!(layout.size() % PAGE_SIZE, 0);
        let next = align_down(self.page_pos - layout.size(), layout.align());
        if next <= self.byte_pos {
            alloc::alloc::handle_alloc_error(layout)
        } else {
            self.page_pos = next;
            NonNull::new(next as *mut u8).ok_or(AllocError::NoMemory)
        }
    }

    pub fn total_pages(&self) -> usize {
        (self.end - self.start) / PAGE_SIZE
    }
    pub fn used_pages(&self) -> usize {
        (self.end - self.page_pos) / PAGE_SIZE
    }
    pub fn available_pages(&self) -> usize {
        (self.page_pos - self.byte_pos) / PAGE_SIZE
    }
}

impl EarlyAllocator {
    pub fn alloc_bytes(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        let start = align_up(self.byte_pos, layout.align());
        let next = start + layout.size();
        if next > self.page_pos {
            alloc::alloc::handle_alloc_error(layout)
        } else {
            self.byte_pos = next;
            self.count += 1;
            NonNull::new(start as *mut u8).ok_or(AllocError::NoMemory)
        }
    }

    pub fn dealloc_bytes(&mut self, _ptr: NonNull<u8>, _layout: Layout) {
        self.count -= 1;
        if self.count == 0 {
            self.byte_pos = self.start;
        }
    }

    fn total_bytes(&self) -> usize {
        self.end - self.start
    }
    fn used_bytes(&self) -> usize {
        self.byte_pos - self.start
    }
    fn available_bytes(&self) -> usize {
        self.page_pos - self.byte_pos
    }
}
```



