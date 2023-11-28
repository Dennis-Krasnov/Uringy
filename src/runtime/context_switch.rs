//! Abstraction over userspace multitasking.
//!
//! Provides an implementation for every CPU architecture.

use std::arch::global_asm;

/// Handle to a stack pointer set up for context switching.
#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub(super) struct Continuation(*const ());

extern "C" {
    /// Initializes a stack for context switching.
    pub(super) fn prepare_stack(stack: *mut u8, func: extern "C" fn() -> !) -> Continuation;

    /// Executes a context switch.
    ///
    /// Spills registers, sets [from] to updated stack pointer.
    /// Sets stack pointer to [to], restores registers.
    pub(super) fn jump(from: *mut Continuation, to: *const Continuation);
}

#[cfg(not(target_arch = "x86_64"))]
compile_error!("Uringy only supports x86_64");

#[cfg(target_arch = "x86_64")]
global_asm!(include_str!("assembly/x86_64.s"));
