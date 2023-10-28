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
//! Downsides:
//! - buffer size has to be a multiple of the page size
//! - takes several syscalls to set up and tear down
//! - occupies several entries in the TLB
//! - doesn't work in no_std
//! - need to handle race conditions on Windows

use std::os::fd::{AsRawFd, FromRawFd};
use std::{ffi, io, ops, ptr, slice};

/// ...
#[derive(Debug)]
pub struct CircularBuffer {
    _file: std::fs::File,
    pointer: *mut u8,
    head: usize,
    tail: usize,
    length: usize,
}

impl CircularBuffer {
    /// ...
    /// minimum [size] in bytes.
    pub fn new(size: usize) -> io::Result<Self> {
        let page_size = if cfg!(feature = "huge_pages") {
            2048 * 1024
        } else {
            unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
        };

        let size = size
            .checked_next_multiple_of(page_size)
            .and_then(usize::checked_next_power_of_two)
            .ok_or(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid circular buffer size",
            ))?;

        // setup physical memory
        let file = anonymous_file()?;
        file.set_len(size as u64)?;

        // setup virtual memory mappings
        let pointer = anonymous_mapping(2 * size)?;
        file_mapping(&file, pointer, size)?;
        file_mapping(&file, unsafe { pointer.add(size) }, size)?;

        Ok(CircularBuffer {
            _file: file,
            pointer: pointer as *mut u8,
            head: 0,
            tail: 0,
            length: size,
        })
    }

    /// ... uninit -> data
    #[inline]
    pub fn commit(&mut self, capacity: usize) {
        assert!(capacity <= self.uninit().len());

        self.tail = self.tail.overflowing_add(capacity).0; // safe to overflow due to power of two size
    }

    /// ... data -> uninit
    #[inline]
    pub fn consume(&mut self, capacity: usize) {
        self.head = self.head.overflowing_add(capacity).0; // safe to overflow due to power of two size

        assert!(self.head <= self.tail);
    }

    /// ...
    #[inline]
    pub fn data(&mut self) -> Buffer<'_> {
        Buffer(unsafe {
            slice::from_raw_parts_mut(
                self.pointer.add(p2_modulo(self.head, self.length)),
                self.tail - self.head,
            )
        })
    }

    /// ...
    #[inline]
    pub fn uninit(&mut self) -> Buffer<'_> {
        Buffer(unsafe {
            slice::from_raw_parts_mut(
                self.pointer.add(p2_modulo(self.tail, self.length)),
                self.length - (self.tail - self.head),
            )
        })
    }

    /// ...
    #[inline]
    pub fn len(&self) -> usize {
        self.length
    }
}

impl Drop for CircularBuffer {
    fn drop(&mut self) {
        let pointer = self.pointer as *mut ffi::c_void;
        let _ = remove_mapping(pointer, 2 * self.length);
        let _ = remove_mapping(pointer, self.length);
        let _ = remove_mapping(unsafe { pointer.add(self.length) }, self.length);
    }
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

/// Bit-hacking optimization for calculating a number mod a power of two.
unsafe fn p2_modulo(n: usize, m: usize) -> usize {
    debug_assert!(m.is_power_of_two());
    n & (m - 1)
}

/// ...
#[derive(Debug)]
pub struct Buffer<'a>(&'a mut [u8]);

impl ops::Deref for Buffer<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl ops::DerefMut for Buffer<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

// TODO: AsRef<[u8]>, AsMut<[u8]>, Borrow<[u8]>, BorrowMut<[u8]>

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initially_empty() {
        let mut queue = CircularBuffer::new(4096).unwrap();

        assert_eq!(queue.data().len(), 0);
        assert_eq!(queue.uninit().len(), queue.len());
    }

    #[test]
    fn rounds_up_size_to_nearest_page_size() {
        let queue = CircularBuffer::new(1).unwrap();

        assert!(queue.len() > 1);
        assert!(queue.len().is_power_of_two());
    }

    #[test]
    fn commit_uninitialized_memory() {
        let mut queue = CircularBuffer::new(4096).unwrap();

        queue.uninit()[..3].copy_from_slice(b"hii");
        queue.commit(3);

        assert_eq!(queue.data().as_ref(), b"hii");
        assert_eq!(queue.uninit().len(), queue.len() - 3);
    }

    #[test]
    fn consume_written_data() {
        let mut queue = CircularBuffer::new(4096).unwrap();
        queue.uninit()[..3].copy_from_slice(b"hii");
        queue.commit(3);

        let consumed = &queue.data()[..2];
        assert_eq!(consumed, b"hi");
        queue.consume(2);

        assert_eq!(queue.data().as_ref(), b"i");
        assert_eq!(queue.uninit().len(), queue.len() - 1);
    }

    #[test]
    fn overflowing_uninitialized() {
        let mut queue = CircularBuffer::new(4096).unwrap();
        queue.commit(queue.len() - 1);
        queue.consume(queue.len() - 1);

        assert_eq!(queue.uninit().len(), queue.len());
    }

    #[test]
    fn overflowing_data() {
        let mut queue = CircularBuffer::new(4096).unwrap();
        queue.commit(queue.len() - 1);
        queue.consume(queue.len() - 1);

        queue.uninit()[..2].copy_from_slice(b"hi");
        queue.commit(2);

        assert_eq!(queue.data().as_ref(), b"hi");
    }

    #[test]
    #[should_panic]
    fn cant_consume_more_than_committed() {
        let mut queue = CircularBuffer::new(4096).unwrap();

        queue.consume(1);
    }

    #[test]
    #[should_panic]
    fn cant_commit_more_than_uninitialized() {
        let mut queue = CircularBuffer::new(4096).unwrap();
        queue.commit(queue.len() - 1);

        queue.commit(2);
    }

    #[test]
    #[ignore]
    fn cleans_up_after_itself() {
        // enough to hit OS limits
        for _ in 0..1_000_000 {
            drop(CircularBuffer::new(4096).unwrap());
        }
    }
}
