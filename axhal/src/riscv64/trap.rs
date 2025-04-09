use super::context::TrapFrame;
use axlog::debug;
use crate_interface::{call_interface, def_interface};
use riscv::register::scause::{self, Trap};

core::arch::global_asm!(
    include_str!("trap.S"),
    trapframe_size = const core::mem::size_of::<TrapFrame>(),
);

/// Writes Supervisor Trap Vector Base Address Register (`stvec`).
#[inline]
pub fn set_trap_vector_base(addr: usize) {
    // Similar to the satp::set approach, use a direct mode setter for stvec
    unsafe {
        // Write the address directly
        core::arch::asm!("csrw stvec, {}", in(reg) addr);
    }
}

#[unsafe(no_mangle)]
fn riscv_trap_handler(tf: &mut TrapFrame) {
    let scause = scause::read();
    match scause.cause() {
        // Use usize constants for Exception codes since we don't have the Exception enum
        Trap::Exception(3) => handle_breakpoint(&mut tf.sepc), // 3 is the standard code for Breakpoint exception
        Trap::Interrupt(_) => handle_irq_extern(scause.bits()),
        _ => {
            panic!(
                "Unhandled trap {:?} @ {:#x}:\n{:#x?}",
                scause.cause(),
                tf.sepc,
                tf
            );
        }
    }
}

fn handle_breakpoint(sepc: &mut usize) {
    debug!("Exception(Breakpoint) @ {:#x} ", sepc);
    *sepc += 2
}

/// Trap handler interface.
///
/// This trait is defined with the [`#[def_interface]`][1] attribute. Users
/// should implement it with [`#[impl_interface]`][2] in any other crate.
///
/// [1]: crate_interface::def_interface
/// [2]: crate_interface::impl_interface
#[def_interface]
pub trait TrapHandler {
    /// Handles interrupt requests for the given IRQ number.
    fn handle_irq(irq_num: usize);
    // more e.g.: handle_page_fault();
}

/// Call the external IRQ handler.
#[allow(dead_code)]
pub(crate) fn handle_irq_extern(irq_num: usize) {
    call_interface!(TrapHandler::handle_irq, irq_num);
}
