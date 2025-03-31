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



## Ch2 
