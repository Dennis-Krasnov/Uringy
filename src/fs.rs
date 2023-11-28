//! Filesystem operations inspired by the standard library.

use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::{cmp, ffi, io, mem};

use io_uring::types::FsyncFlags;

use crate::runtime;

/// Handle to an open file.
pub struct File(RawFd);

impl File {
    /// Opens a file in read-only mode.
    pub fn open<P: AsRef<Path>>(path: P) -> crate::IoResult<Self> {
        OpenOptions::new().read(true).open(path.as_ref())
    }

    /// Opens a file in write-only mode.
    pub fn create(path: impl AsRef<Path>) -> crate::IoResult<Self> {
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path.as_ref())
    }

    /// Returns an [OpenOptions] builder.
    /// Equivalent to [OpenOptions::new()].
    #[must_use]
    pub fn options() -> OpenOptions {
        OpenOptions::new()
    }

    /// Syncs all OS-internal metadata to disk.
    /// Catches errors that would otherwise be ignored when dropping the file.
    pub fn sync_all(&self) -> crate::IoResult<()> {
        let fd = io_uring::types::Fd(self.0);
        let sqe = io_uring::opcode::Fsync::new(fd).build();
        let result = runtime::syscall(sqe)?;
        assert_eq!(result, 0);

        Ok(())
    }

    /// Syncs content, but maybe not file metadata to disk.
    /// Reduces disk operations compared to [sync_all].
    pub fn sync_data(&self) -> crate::IoResult<()> {
        let fd = io_uring::types::Fd(self.0);
        let sqe = io_uring::opcode::Fsync::new(fd)
            .flags(FsyncFlags::DATASYNC)
            .build();
        let result = runtime::syscall(sqe)?;
        assert_eq!(result, 0);

        Ok(())
    }

    /// Truncates or extends the underlying file.
    pub fn set_len(&self, size: u64) -> crate::IoResult<()> {
        let file = unsafe { std::fs::File::from_raw_fd(self.0) };
        file.set_len(size)?;
        mem::forget(file);

        Ok(())
    }

    /// Queries metadata about the underlying file.
    pub fn metadata(&self) -> crate::IoResult<std::fs::Metadata> {
        let file = unsafe { std::fs::File::from_raw_fd(self.0) };
        let metadata = file.metadata()?;
        mem::forget(file);

        // TODO io_uring operation

        Ok(metadata)
    }

    // /// ...
    // pub fn try_clone(&self) -> crate::IoResult<File> {
    //
    // }

    /// Changes the permissions on the underlying file.
    pub fn set_permissions(&self, permissions: std::fs::Permissions) -> crate::IoResult<()> {
        let file = unsafe { std::fs::File::from_raw_fd(self.0) };
        file.set_permissions(permissions)?;
        mem::forget(file);

        Ok(())
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let fd = io_uring::types::Fd(self.0);
        let sqe = io_uring::opcode::Close::new(fd).build();
        let _ = runtime::syscall(sqe);
    }
}

// TODO: doesn't work if using fixed fd
impl FromRawFd for File {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        File(fd)
    }
}

impl AsRawFd for File {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

// impl IntoRawFd for File {
//     fn into_raw_fd(self) -> RawFd {
//         FIXME: decide whether to close or not
//         self.0
//     }
// }

// The maximum read limit on most POSIX-like systems is `SSIZE_MAX`,
// with the man page quoting that if the count of bytes to read is
// greater than `SSIZE_MAX` the result is "unspecified".
//
// On macOS, however, apparently the 64-bit libc is either buggy or
// intentionally showing odd behavior by rejecting any read with a size
// larger than or equal to INT_MAX. To handle both of these the read
// size is capped on both platforms.
#[cfg(target_os = "macos")]
const READ_LIMIT: u32 = libc::c_int::MAX as u32 - 1;
#[cfg(not(target_os = "macos"))]
const READ_LIMIT: u32 = libc::ssize_t::MAX as u32;

impl io::Write for File {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let fd = io_uring::types::Fd(self.0);
        let sqe =
            io_uring::opcode::Write::new(fd, buf.as_ptr(), cmp::min(buf.len() as u32, READ_LIMIT))
                .offset(0_u64.wrapping_sub(1)) // use file offset for files that support seeking
                .build();
        let bytes_wrote = runtime::syscall(sqe)?;
        Ok(bytes_wrote as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl io::Read for File {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let fd = io_uring::types::Fd(self.0);
        let sqe = io_uring::opcode::Read::new(
            fd,
            buf.as_mut_ptr(),
            cmp::min(buf.len() as u32, READ_LIMIT),
        )
        .offset(0_u64.wrapping_sub(1)) // use file offset for files that support seeking
        .build();
        let bytes_read = runtime::syscall(sqe)?;
        Ok(bytes_read as usize)
    }
}

/// Options and flags for configuring how a file is opened.
#[derive(Clone, Debug)]
pub struct OpenOptions {
    // generic
    read: bool,
    write: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
    // system-specific
    custom_flags: i32,
    mode: libc::mode_t,
}

impl OpenOptions {
    /// Creates a blank new set of options ready for configuration.
    ///
    /// All options are initially set to false.
    pub fn new() -> Self {
        OpenOptions {
            read: false,
            write: false,
            append: false,
            truncate: false,
            create: false,
            create_new: false,
            custom_flags: 0,
            mode: 0o666, // read and write but not execute
        }
    }

    /// Sets the option for read access.
    pub fn read(&mut self, read: bool) -> &mut Self {
        self.read = read;
        self
    }

    /// Sets the option for write access.
    ///
    /// If the file already exists, any write calls on it will overwrite its contents, without truncating it.
    pub fn write(&mut self, write: bool) -> &mut Self {
        self.write = write;
        self
    }

    /// Sets the option for the append mode.
    ///
    /// Note that setting [.write(true).append(true)] has the same effect as setting only [.append(true)].
    ///
    /// ## Note
    /// This function doesn't create the file if it doesn't exist.
    /// Use the [`OpenOptions::create`] method to do so.
    pub fn append(&mut self, append: bool) -> &mut Self {
        self.append = append;
        self
    }

    /// Sets the option for truncating a previous file.
    ///
    /// The file must be opened with write access for truncate to work.
    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.truncate = truncate;
        self
    }

    /// Sets the option to create a new file, or open it if it already exists.
    ///
    /// In order for the file to be created, [OpenOptions::write] or [OpenOptions::append] access must be used.
    ///
    /// See also [uringy::fs::write()] for a simple function to create a file with a given data.
    pub fn create(&mut self, create: bool) -> &mut Self {
        self.create = create;
        self
    }

    /// Sets the option to create a new file, failing if it already exists.
    ///
    /// This option is useful because it is atomic.
    ///
    /// If .create_new(true) is set, .create() and .truncate() are ignored.
    ///
    /// The file must be opened with write or append access in order to create a new file.
    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.create_new = create_new;
        self
    }

    fn get_access_mode(&self) -> io::Result<libc::c_int> {
        match (self.read, self.write, self.append) {
            (true, false, false) => Ok(libc::O_RDONLY),
            (false, true, false) => Ok(libc::O_WRONLY),
            (true, true, false) => Ok(libc::O_RDWR),
            (false, _, true) => Ok(libc::O_WRONLY | libc::O_APPEND),
            (true, _, true) => Ok(libc::O_RDWR | libc::O_APPEND),
            (false, false, false) => Err(io::Error::from_raw_os_error(libc::EINVAL)),
        }
    }

    fn get_creation_mode(&self) -> io::Result<libc::c_int> {
        match (self.write, self.append) {
            (true, false) => {}
            (false, false) => {
                if self.truncate || self.create || self.create_new {
                    return Err(io::Error::from_raw_os_error(libc::EINVAL));
                }
            }
            (_, true) => {
                if self.truncate && !self.create_new {
                    return Err(io::Error::from_raw_os_error(libc::EINVAL));
                }
            }
        }

        Ok(match (self.create, self.truncate, self.create_new) {
            (false, false, false) => 0,
            (true, false, false) => libc::O_CREAT,
            (false, true, false) => libc::O_TRUNC,
            (true, true, false) => libc::O_CREAT | libc::O_TRUNC,
            (_, _, true) => libc::O_CREAT | libc::O_EXCL,
        })
    }

    /// Opens a file at [path] with the options specified by [self].
    pub fn open(&self, path: impl AsRef<Path>) -> crate::IoResult<File> {
        let fd = io_uring::types::Fd(libc::AT_FDCWD); // pathname is relative to working directory
        let path = ffi::CString::new(path.as_ref().as_os_str().as_bytes()).unwrap();
        let flags = libc::O_CLOEXEC
            | self.get_access_mode()?
            | self.get_creation_mode()?
            | (self.custom_flags as libc::c_int & !libc::O_ACCMODE);
        let sqe = io_uring::opcode::OpenAt::new(fd, path.as_ptr())
            .mode(self.mode)
            .flags(flags)
            .build();
        runtime::syscall(sqe).map(|fd| File(fd as i32))
    }
}

/// Copies the contents of one file to another.
/// This function will also copy the permission bits of the original file to the destination file.
///
/// This function will **overwrite** the contents of `to`.
///
/// Note that if `from` and `to` both point to the same file, then the file will likely get truncated by this operation.
///
/// On success, the total number of bytes copied is returned and it is equal to the length of the `to` file as reported by `metadata`.
///
/// If you want to copy the contents of one file to another and you’re working with [`File`]s, see the [`io::copy()`] function.
pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> crate::IoResult<u64> {
    std::fs::copy(from.as_ref(), to.as_ref()).map_err(crate::Error::from_io_error)
}

/// Queries metadata about the underlying file.
pub fn metadata(path: impl AsRef<Path>) -> crate::IoResult<std::fs::Metadata> {
    let file = File::open(path.as_ref())?;
    file.metadata()
}

/// Read the entire contents of a file into a bytes vector.
pub fn read(path: impl AsRef<Path>) -> crate::IoResult<Vec<u8>> {
    let mut file = File::open(path.as_ref())?;
    let mut vector = vec![];
    file.read_to_end(&mut vector)
        .map_err(crate::Error::from_io_error)?;

    Ok(vector)
}

/// Read the entire contents of a file into a string.
pub fn read_to_string(path: impl AsRef<Path>) -> crate::IoResult<String> {
    let file = File::open(path.as_ref())?;
    let string = io::read_to_string(file).map_err(crate::Error::from_io_error)?;

    Ok(string)
}

/// Removes a file from the filesystem.
pub fn remove_file(path: impl AsRef<Path>) -> crate::IoResult<()> {
    let fd = io_uring::types::Fd(libc::AT_FDCWD); // pathname is relative to working directory
    let path = ffi::CString::new(path.as_ref().as_os_str().as_bytes()).unwrap();
    let sqe = io_uring::opcode::UnlinkAt::new(fd, path.as_ptr()).build();
    let result = runtime::syscall(sqe)?;
    assert_eq!(result, 0);

    Ok(())
}

/// Write a slice as the entire contents of a file.
pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> crate::IoResult<()> {
    File::create(path.as_ref())?
        .write_all(contents.as_ref())
        .map_err(crate::Error::from_io_error)?;

    Ok(())
}

// TODO: O_LARGEFILE open64, otherwise EOVERFLOW
// TODO: https://docs.rs/io-uring/latest/io_uring/opcode/struct.MkDirAt.html

#[cfg(test)]
mod tests {
    use crate::runtime::start;

    use super::*;

    #[test]
    fn creates_and_deletes_file() {
        start(|| {
            let path = format!("/tmp/{}", uuid::Uuid::new_v4());

            File::create(&path).unwrap();
            assert!(Path::new(&path).exists());

            remove_file(&path).unwrap();
            assert!(!Path::new(&path).exists());
        })
        .unwrap();
    }

    #[test]
    fn copies_file() {
        start(|| {
            let path = format!("/tmp/{}", uuid::Uuid::new_v4());

            copy("/etc/hosts", &path).unwrap();

            assert_eq!(read("/etc/hosts").unwrap(), read(&path).unwrap());
        })
        .unwrap();
    }

    #[test]
    fn truncates_file() {
        start(|| {
            let path = format!("/tmp/{}", uuid::Uuid::new_v4());
            write(&path, b"hi").unwrap();

            write(&path, b"hello").unwrap();

            assert_eq!(read(&path).unwrap(), b"hello");
        })
        .unwrap();
    }

    #[test]
    fn appends_to_file() {
        start(|| {
            let path = format!("/tmp/{}", uuid::Uuid::new_v4());
            write(&path, b"hi ").unwrap();

            let mut file = File::options().append(true).open(&path).unwrap();
            file.write_all(b"hello").unwrap();

            assert_eq!(read(&path).unwrap(), b"hi hello");
        })
        .unwrap();
    }

    #[test]
    fn queries_metadata() {
        start(|| {
            let uringy = metadata("/etc/hosts").unwrap();
            let std = std::fs::metadata("/etc/hosts").unwrap();

            // core
            assert_eq!(uringy.file_type(), std.file_type());
            assert_eq!(uringy.is_dir(), std.is_dir());
            assert_eq!(uringy.is_file(), std.is_file());
            assert_eq!(uringy.is_symlink(), std.is_symlink());
            assert_eq!(uringy.len(), std.len());
            assert_eq!(uringy.permissions(), std.permissions());
            // assert_eq!(uringy.modified(), std.modified());
            // assert_eq!(uringy.accessed(), std.accessed());
            // assert_eq!(uringy.created(), std.created());

            {
                use std::os::unix::fs::MetadataExt;

                assert_eq!(uringy.dev(), std.dev());
                assert_eq!(uringy.ino(), std.ino());
                assert_eq!(uringy.mode(), std.mode());
                assert_eq!(uringy.nlink(), std.nlink());
                assert_eq!(uringy.uid(), std.uid());
                assert_eq!(uringy.gid(), std.gid());
                assert_eq!(uringy.rdev(), std.rdev());
                assert_eq!(uringy.size(), std.size());
                assert_eq!(uringy.atime(), std.atime());
                assert_eq!(uringy.atime_nsec(), std.atime_nsec());
                assert_eq!(uringy.mtime(), std.mtime());
                assert_eq!(uringy.mtime_nsec(), std.mtime_nsec());
                assert_eq!(uringy.ctime(), std.ctime());
                assert_eq!(uringy.ctime_nsec(), std.ctime_nsec());
                assert_eq!(uringy.blksize(), std.blksize());
                assert_eq!(uringy.blocks(), std.blocks());
            }

            {
                use std::os::linux::fs::MetadataExt;

                // assert_eq!(uringy.as_raw_stat(), std.as_raw_stat());
                assert_eq!(uringy.st_dev(), std.st_dev());
                assert_eq!(uringy.st_ino(), std.st_ino());
                assert_eq!(uringy.st_mode(), std.st_mode());
                assert_eq!(uringy.st_nlink(), std.st_nlink());
                assert_eq!(uringy.st_uid(), std.st_uid());
                assert_eq!(uringy.st_gid(), std.st_gid());
                assert_eq!(uringy.st_rdev(), std.st_rdev());
                assert_eq!(uringy.st_size(), std.st_size());
                assert_eq!(uringy.st_atime(), std.st_atime());
                assert_eq!(uringy.st_atime_nsec(), std.st_atime_nsec());
                assert_eq!(uringy.st_mtime(), std.st_mtime());
                assert_eq!(uringy.st_mtime_nsec(), std.st_mtime_nsec());
                assert_eq!(uringy.st_ctime(), std.st_ctime());
                assert_eq!(uringy.st_ctime_nsec(), std.st_ctime_nsec());
                assert_eq!(uringy.st_blksize(), std.st_blksize());
                assert_eq!(uringy.st_blocks(), std.st_blocks());
            }
        })
        .unwrap();
    }
}
