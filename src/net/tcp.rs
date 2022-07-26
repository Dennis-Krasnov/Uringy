//! Reliable, connection-oriented transport.

use crate::runtime;
use std::marker::PhantomData;
use std::mem;
use std::mem::MaybeUninit;
use std::net::ToSocketAddrs;
use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};

/// ...
#[derive(Debug)]
pub struct TcpStream {
    fd: RawFd,
    _marker: PhantomData<*const ()>,
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        let sync_tcp = unsafe { std::net::TcpStream::from_raw_fd(self.fd) };
        drop(sync_tcp);
    }
}

impl TcpStream {
    /// ...
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> std::io::Result<TcpStream> {
        // TODO: asynchronously connect, had problems with ToSocketAddrs
        let sync_tcp = std::net::TcpStream::connect(addr)?;
        Ok(TcpStream {
            fd: sync_tcp.into_raw_fd(),
            _marker: PhantomData,
        })
    }

    /// ...
    pub async unsafe fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        runtime::syscall(
            runtime::io_uring::opcode::Read::new(
                runtime::io_uring::types::Fd(self.fd),
                buffer.as_mut_ptr(),
                buffer.len() as _,
            )
            .build(),
        )
        .await
        .map(|bytes_read| bytes_read as usize)
    }

    /// ...
    ///
    /// # Safety
    /// File descriptor and buffer must be valid for the duration of the syscall.
    /// eg. static memory, or just ensuring to call .await (even if called on background task that didn't run to completion because root task finished)
    pub async unsafe fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        runtime::syscall(
            runtime::io_uring::opcode::Write::new(
                runtime::io_uring::types::Fd(self.fd),
                buffer.as_ptr(),
                buffer.len() as _,
            )
            .build(),
        )
        .await
        .map(|bytes_wrote| bytes_wrote as usize)
    }

    /// ...
    pub fn try_clone(&self) -> std::io::Result<Self> {
        let sync_tcp = unsafe { std::net::TcpStream::from_raw_fd(self.fd) };
        let sync_tcp_copy = sync_tcp.try_clone()?;
        mem::forget(sync_tcp);
        Ok(TcpStream {
            fd: sync_tcp_copy.into_raw_fd(),
            _marker: PhantomData,
        })
    }

    /// ...
    pub async fn close(self) -> std::io::Result<()> {
        let result = runtime::syscall(
            runtime::io_uring::opcode::Close::new(runtime::io_uring::types::Fd(self.fd)).build(),
        )
        .await
        .map(|_zero| ());

        // Don't run drop
        mem::forget(self);

        result
    }
}

/// ...
#[derive(Debug)]
pub struct TcpListener {
    fd: RawFd,
    _marker: PhantomData<*const ()>,
}

impl TcpListener {
    /// ...
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> std::io::Result<TcpListener> {
        let sync_tcp = std::net::TcpListener::bind(addr)?;
        Ok(TcpListener {
            fd: sync_tcp.into_raw_fd(),
            _marker: PhantomData,
        })
    }

    /// ...
    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        let sync_tcp = unsafe { std::net::TcpListener::from_raw_fd(self.fd) };
        let address = sync_tcp.local_addr();
        mem::forget(sync_tcp);
        address
    }

    /// ...
    pub async fn accept(&self) -> std::io::Result<TcpStream> {
        let mut addr: MaybeUninit<libc::sockaddr_storage> = MaybeUninit::uninit();
        let mut length = mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;

        let file_descriptor = runtime::syscall(
            runtime::io_uring::opcode::Accept::new(
                runtime::io_uring::types::Fd(self.fd),
                addr.as_mut_ptr() as *mut _,
                &mut length,
            )
            .build(),
        )
        .await?;

        Ok(TcpStream {
            fd: file_descriptor as RawFd,
            _marker: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::block_on;
    use std::io::{Read, Write};
    use std::sync::mpsc;

    #[test]
    fn client() {
        let (tx, rx) = mpsc::channel();

        let join_handle = std::thread::spawn(move || {
            let tcp_listener = std::net::TcpListener::bind("localhost:0").unwrap();
            tx.send(tcp_listener.local_addr().unwrap()).unwrap();

            let (mut tcp_stream, _) = tcp_listener.accept().unwrap();
            let mut buffer = [0; 1024];
            let bytes_read = tcp_stream.read(&mut buffer).unwrap();
            assert_eq!(buffer[..bytes_read], b"hello"[..]);
        });

        let address = rx.recv().unwrap();

        block_on(async move {
            let mut tcp_stream = TcpStream::connect(address).await.unwrap();
            let bytes_wrote = unsafe { tcp_stream.write(b"hello") }.await.unwrap();
            assert_eq!(bytes_wrote, "hello".len());
        });

        join_handle.join().unwrap();
    }

    #[test]
    fn server() {
        let (tx, rx) = mpsc::channel();

        let join_handle = std::thread::spawn(move || {
            block_on(async move {
                let tcp_listener = TcpListener::bind("localhost:0").await.unwrap();
                tx.send(tcp_listener.local_addr().unwrap()).unwrap();

                let mut tcp_stream = tcp_listener.accept().await.unwrap();
                let mut buffer = [0; 1024];
                let bytes_read = unsafe { tcp_stream.read(&mut buffer) }.await.unwrap();
                assert_eq!(buffer[..bytes_read], b"hello"[..]);
            });
        });

        let address = rx.recv().unwrap();

        let mut tcp_stream = std::net::TcpStream::connect(address).unwrap();
        let bytes_wrote = tcp_stream.write(b"hello").unwrap();
        assert_eq!(bytes_wrote, "hello".len());

        join_handle.join().unwrap();
    }

    #[test]
    fn trait_implementations() {
        use impls::impls;
        use std::fmt::Debug;

        assert!(impls!(TcpStream: Debug & !Send & !Sync & !Clone));
        assert!(impls!(TcpListener: Debug & !Send & !Sync & !Clone));
    }
}
