//! ...

use std::arch::global_asm;

/// ...
#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub(super) struct Continuation(*const ());

extern "C" {
    /// ...
    pub(super) fn prepare_stack(stack: *mut u8, func: *const ()) -> Continuation;

    /// ...
    pub(super) fn jump(to: Continuation, save: *mut Continuation);
}

#[cfg(not(target_arch = "x86_64"))]
compile_error!("Uringy only supports x86_64");

#[cfg(target_arch = "x86_64")]
global_asm!(include_str!("assembly/x86_64.s"));
