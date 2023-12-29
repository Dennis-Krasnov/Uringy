//! Circular buffer data structure for byte streams.
//!
//! A virtual memory trick is used to efficiently implement the circular buffer.
//! Two consecutive blocks of virtual memory are mapped to the same physical memory.
//! Consumers are able to read the full message despite it being split between the end and start of the buffer.
//! Producers can also transparently write into both the end and start of the buffer.
//!
//! physical memory: D E 0 0 0 0 A B C
//!                    ^tail     ^head
//!
//! virtual memory:  D E 0 0 0 0 A B C D E 0 0 0 0 A B C
//!                              \-------/ continuous
//!
//! 2MB huge pages can be enabled with the [huge_pages] cargo feature.
//! https://www.kernel.org/doc/Documentation/admin-guide/mm/hugetlbpage.rst
//!
//! This design has several downsides:
//! - Buffer size has to be a multiple of the page size (4KB).
//! - It takes several blocking syscalls to set up and tear down (~16μs).
//! - Memory maps occupy several entries in the TLB.
//! - It doesn't work in `no_std` environments.

use std::cell::RefCell;
use std::os::fd::{AsRawFd, FromRawFd};
use std::rc::Rc;
use std::{ffi, io, ops, ptr, slice};

/// ...
/// minimum [length] in bytes.
pub fn circular_buffer(length: usize) -> io::Result<(Data, Uninit)> {
    let length = calculate_length(length)?;

    // setup physical memory
    let file = anonymous_file()?;
    file.set_len(length as u64)?;

    // setup virtual memory mappings
    let pointer = anonymous_mapping(2 * length)?;
    file_mapping(&file, pointer, length)?;
    file_mapping(&file, unsafe { pointer.byte_add(length) }, length)?;

    let state = Rc::new(RefCell::new(State {
        _file: file,
        pointer,
        head: 0,
        tail: 0,
        length,
    }));

    Ok((Data(state.clone()), Uninit(state)))
}

fn calculate_length(length: usize) -> io::Result<usize> {
    let page_size = if cfg!(feature = "huge_pages") {
        2 * 1024 * 1024
    } else {
        unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
    };

    length
        .checked_next_multiple_of(page_size)
        .and_then(usize::checked_next_power_of_two)
        .ok_or(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid circular buffer size",
        ))
}

/// ...
#[derive(Debug)]
pub struct Data(Rc<RefCell<State>>);

impl Data {
    /// ... data -> uninit
    pub fn consume(&mut self, capacity: usize) {
        let mut state = self.0.borrow_mut();
        state.head = state.head.overflowing_add(capacity).0; // safe to overflow due to power of two length
        assert!(state.head <= state.tail);
    }

    /// ...
    pub fn len(&self) -> usize {
        let state = self.0.borrow();
        state.data_len()
    }

    /// ...
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl ops::Deref for Data {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        let state = self.0.borrow();
        unsafe {
            slice::from_raw_parts(
                state.pointer.byte_add(p2_modulo(state.head, state.length)) as *const u8,
                state.data_len(),
            )
        }
    }
}

impl ops::DerefMut for Data {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let state = self.0.borrow_mut();
        unsafe {
            slice::from_raw_parts_mut(
                state.pointer.byte_add(p2_modulo(state.head, state.length)) as *mut u8,
                state.data_len(),
            )
        }
    }
}

/// ...
#[derive(Debug)]
pub struct Uninit(Rc<RefCell<State>>);

impl Uninit {
    /// ... uninit -> data
    pub fn commit(&mut self, capacity: usize) {
        assert!(capacity <= self.len());
        let mut state = self.0.borrow_mut();
        state.tail = state.tail.overflowing_add(capacity).0; // safe to overflow due to power of two length
    }

    /// ...
    pub fn len(&self) -> usize {
        let state = self.0.borrow();
        state.uninit_len()
    }

    /// ...
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl ops::Deref for Uninit {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        let state = self.0.borrow();
        unsafe {
            slice::from_raw_parts(
                state.pointer.byte_add(p2_modulo(state.tail, state.length)) as *const u8,
                state.uninit_len(),
            )
        }
    }
}

impl ops::DerefMut for Uninit {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let state = self.0.borrow_mut();
        unsafe {
            slice::from_raw_parts_mut(
                state.pointer.byte_add(p2_modulo(state.tail, state.length)) as *mut u8,
                state.uninit_len(),
            )
        }
    }
}

#[derive(Debug)]
struct State {
    _file: std::fs::File,
    pointer: *mut ffi::c_void,
    head: usize,
    tail: usize,
    length: usize,
}

impl State {
    fn data_len(&self) -> usize {
        self.tail - self.head
    }

    fn uninit_len(&self) -> usize {
        self.length - self.data_len()
    }
}

impl Drop for State {
    fn drop(&mut self) {
        let _ = remove_mapping(self.pointer, 2 * self.length);
        let _ = remove_mapping(self.pointer, self.length);
        let _ = remove_mapping(unsafe { self.pointer.byte_add(self.length) }, self.length);
    }
}

/// Bit-hacking optimization for calculating a number mod a power of two.
unsafe fn p2_modulo(n: usize, m: usize) -> usize {
    debug_assert!(m.is_power_of_two());
    n & (m - 1)
}

fn anonymous_file() -> io::Result<std::fs::File> {
    let mut flags = libc::MFD_CLOEXEC;
    if cfg!(feature = "huge_pages") {
        flags |= libc::MFD_HUGETLB;
    }

    let fd = unsafe { libc::memfd_create(b"circular-buffer\0".as_ptr() as _, flags) };
    if fd == -1 {
        let error = io::Error::last_os_error();
        return Err(error);
    }

    let file = unsafe { std::fs::File::from_raw_fd(fd) };

    Ok(file)
}

fn anonymous_mapping(size: usize) -> io::Result<*mut ffi::c_void> {
    let mut flags = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS;
    if cfg!(feature = "huge_pages") {
        flags |= libc::MAP_HUGETLB;
    }

    let pointer = unsafe { libc::mmap(ptr::null_mut(), size, libc::PROT_NONE, flags, -1, 0) };

    if pointer == libc::MAP_FAILED {
        let error = io::Error::last_os_error();
        return Err(error);
    }

    Ok(pointer)
}

fn remove_mapping(pointer: *mut ffi::c_void, size: usize) -> io::Result<()> {
    let result = unsafe { libc::munmap(pointer, size) };
    if result == -1 {
        let error = io::Error::last_os_error();
        return Err(error);
    }

    Ok(())
}

fn file_mapping(file: &std::fs::File, pointer: *mut ffi::c_void, size: usize) -> io::Result<()> {
    let mut flags = libc::MAP_SHARED | libc::MAP_FIXED;
    if cfg!(feature = "huge_pages") {
        flags |= libc::MAP_HUGETLB;
    }

    let pointer = unsafe {
        libc::mmap(
            pointer,
            size,
            libc::PROT_READ | libc::PROT_WRITE,
            flags,
            file.as_raw_fd(),
            0,
        )
    };

    if pointer == libc::MAP_FAILED {
        let error = io::Error::last_os_error();
        return Err(error);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_uninitialized() {
        let (data, uninit) = circular_buffer(4096).unwrap();

        assert!(data.is_empty());
        assert_eq!(uninit.len(), 4096);
    }

    #[test]
    fn rounds_up_length_to_nearest_page_size() {
        let (_, uninit) = circular_buffer(1).unwrap();

        assert!(uninit.len() > 1);
        assert!(uninit.len().is_power_of_two());
    }

    #[test]
    fn commits_uninit() {
        let (data, mut uninit) = circular_buffer(4096).unwrap();

        uninit[..2].copy_from_slice(b"hi");
        uninit.commit(2);

        assert_eq!(data.as_ref(), b"hi");
        assert_eq!(uninit.len(), 4096 - 2);
    }

    #[test]
    fn consumes_data() {
        let (mut data, mut uninit) = circular_buffer(4096).unwrap();
        uninit[..2].copy_from_slice(b"hi");
        uninit.commit(2);

        assert_eq!(&data[..1], b"h");
        data.consume(1);

        assert_eq!(data.as_ref(), b"i");
        assert_eq!(uninit.len(), 4096 - 1);
    }

    #[test]
    fn data_spans_across_boundary() {
        let (mut data, mut uninit) = circular_buffer(4096).unwrap();
        uninit.commit(uninit.len() - 1);
        data.consume(data.len());

        uninit[..2].copy_from_slice(b"hi");
        uninit.commit(2);

        assert_eq!(data.as_ref(), b"hi");
    }

    #[test]
    fn uninit_spans_across_boundary() {
        let (mut data, mut uninit) = circular_buffer(4096).unwrap();

        uninit.commit(42);
        data.consume(42);

        assert_eq!(uninit.len(), 4096);
    }

    #[test]
    #[should_panic]
    fn cant_consume_more_than_committed() {
        let (mut data, _) = circular_buffer(4096).unwrap();

        data.consume(data.len() + 1);
    }

    #[test]
    #[should_panic]
    fn cant_commit_more_than_uninitialized() {
        let (_, mut uninit) = circular_buffer(4096).unwrap();

        uninit.commit(uninit.len() + 1);
    }

    #[test]
    #[ignore = "takes 16s to run in release mode"]
    fn cleans_up_after_itself() {
        // enough to hit OS limits
        for _ in 0..1_000_000 {
            drop(circular_buffer(4096).unwrap());
        }
    }
}
