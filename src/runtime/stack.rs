//! ...

use std::num::NonZeroUsize;
use std::{ffi, io, ptr};

#[derive(Debug)]
pub(super) struct Stack {
    pub(super) pointer: *mut u8,
    pub(super) length: usize,
}

impl Stack {
    /// Allocates a general purpose stack.
    /// Demand paging ensures that physical memory is allocated only as necessary, during a page fault.
    /// The stack is protected from overflow using guard pages.
    pub(super) fn new(guard_pages: NonZeroUsize, usable_pages: NonZeroUsize) -> io::Result<Self> {
        let (guard_pages, usable_pages) = (guard_pages.get(), usable_pages.get());

        // page aligned sizes
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize };
        assert_eq!(page_size, 4096);
        let length = (guard_pages + usable_pages) * page_size;

        // kernel allocates an unused block of virtual memory
        let pointer = unsafe {
            libc::mmap(
                ptr::null_mut(),
                length,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        if pointer == libc::MAP_FAILED {
            let error = io::Error::last_os_error();
            return Err(error);
        }

        // if guarding memory goes wrong then mmap gets cleaned up in Stack's drop
        let stack = Stack {
            pointer: pointer as *mut u8,
            length,
        };

        // located at the lowest addresses since the stack grows downward
        let result = unsafe { libc::mprotect(pointer, guard_pages * page_size, libc::PROT_NONE) };
        if result == -1 {
            let error = io::Error::last_os_error();
            return Err(error);
        }

        Ok(stack)
    }

    /// ...
    pub(super) fn base(&self) -> *mut u8 {
        // safety: part of same allocation, can't overflow
        unsafe { self.pointer.add(self.length) }
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        let result = unsafe { libc::munmap(self.pointer as *mut ffi::c_void, self.length) };
        assert_eq!(result, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes() {
        let stack = Stack::new(NonZeroUsize::MIN, NonZeroUsize::MIN).unwrap();
        let pointer = stack.base() as *mut u8;
        unsafe {
            let pointer = pointer.sub(1);
            pointer.write(123);
            assert_eq!(pointer.read(), 123);
        }
    }

    #[test]
    fn cant_execute() {
        // TODO
    }

    #[test]
    #[ignore = "aborts process"] // TODO: test with fork()
    fn overflow() {
        let stack = Stack::new(NonZeroUsize::MIN, NonZeroUsize::MIN).unwrap();
        let pointer = stack.base() as *mut u8;
        unsafe {
            let pointer = pointer.sub(4096 + 1);
            pointer.write(123);
        }
    }
}
