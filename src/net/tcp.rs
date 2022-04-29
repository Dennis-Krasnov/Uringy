//! Reliable, connection-oriented transport.

use crate::runtime;
use crate::utils::range_from_bounds;
use std::mem;
use std::net::ToSocketAddrs;
use std::ops::RangeBounds;
use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};

/// ...
#[derive(Debug)]
pub struct TcpStream {
    fd: RawFd,
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        // TODO: in a new task, best effort, asynchronously close the socket
        let sync_tcp = unsafe { std::net::TcpStream::from_raw_fd(self.fd) };
        drop(sync_tcp);
    }
}

impl TcpStream {
    /// ...
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> std::io::Result<TcpStream> {
        // TODO: asynchronously connect
        let sync_tcp = std::net::TcpStream::connect(addr)?;
        Ok(TcpStream {
            fd: sync_tcp.into_raw_fd(),
        })
    }

    /// ...
    pub async fn read(
        mut self,
        mut buffer: Vec<u8>,
        range_bounds: impl RangeBounds<usize>,
    ) -> (TcpStream, Vec<u8>, std::io::Result<usize>) {
        let range = range_from_bounds(range_bounds, 0, buffer.len());
        // Safety: Owned file descriptor and buffer outlive the syscall
        let result = unsafe { self.read_unchecked(&mut buffer[range]) }.await;
        (self, buffer, result)
    }

    /// ...
    pub async unsafe fn read_unchecked(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        // TODO: replace println with tracing
        println!("reading from TCP CHANNEL");
        runtime::syscall(
            io_uring::opcode::Read::new(
                io_uring::types::Fd(self.fd),
                buffer.as_mut_ptr(),
                buffer.len() as _,
            )
            .build(),
        )
        // runtime::syscall(|sqe| unsafe { sqe.prep_read(self.fd, buffer, 0) })
        // runtime::syscall(|sqe| unsafe { sqe.prep_recv(self.fd, buffer, MsgFlags::empty()) }) // FIXME: doesn't work...
        .await
        // .map(|bytes_read| bytes_read as usize)
        .map(|bytes_read| {
            println!("finished reading: {bytes_read}");
            bytes_read as usize
        })
    }

    /// ...
    // Explicit conversion, don't want to accidentally copy. consider send_unchecked.
    // let (_tcp_stream, _buffer, result) = tcp_stream.send(str.into(), ..).await;
    // Box<u8> instead of Vec<u8> since no use for capacity. NVM. I need vec for indexing (conversion into slice)
    pub async fn write(
        mut self,
        buffer: Vec<u8>,
        range_bounds: impl RangeBounds<usize>,
    ) -> (TcpStream, Vec<u8>, std::io::Result<usize>) {
        let range = range_from_bounds(range_bounds, 0, buffer.len());
        // Safety: Owned file descriptor and buffer outlive the syscall
        let result = unsafe { self.write_unchecked(&buffer[range]) }.await;
        (self, buffer, result)
    }

    /// ...
    ///
    /// # Safety
    /// File descriptor and buffer must be valid for the duration of the syscall.
    /// eg. static memory, or just ensuring to call .await (even if called on background task that didn't run to completion because root task finished)
    pub async unsafe fn write_unchecked(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        println!("writing to TCP CHANNEL");
        // runtime::syscall(|sqe| unsafe { sqe.prep_send(self.fd, buffer, MsgFlags::empty()) })
        runtime::syscall(
            io_uring::opcode::Write::new(
                io_uring::types::Fd(self.fd),
                buffer.as_ptr(),
                buffer.len() as _,
            )
            .build(),
        )
        .await
        .map(|bytes_wrote| {
            println!("finished writing: {bytes_wrote}");
            bytes_wrote as usize
        })
    }

    /// ...
    pub async fn close(self) -> std::io::Result<()> {
        println!("closing TCP CHANNEL");
        // runtime::syscall(|sqe| unsafe { sqe.prep_close(self.fd) })
        let result =
            runtime::syscall(io_uring::opcode::Close::new(io_uring::types::Fd(self.fd)).build())
                .await
                // .map(|_zero| ())
                .map(|_bytes_wrote| {
                    println!("finished closing");
                    ()
                });

        // Don't run drop
        mem::forget(self);

        result
    }
}

// TODO: implement these by converting to std::net::tcpstream, running the function, then forgetting it :)
// pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
//     setsockopt(self, libc::IPPROTO_TCP, libc::TCP_NODELAY, nodelay as c_int)
// }
//
// pub fn nodelay(&self) -> io::Result<bool> {
//     let raw: c_int = getsockopt(self, libc::IPPROTO_TCP, libc::TCP_NODELAY)?;
//     Ok(raw != 0)
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::block_on;
    use std::io::Read;

    mod tcp_stream {
        use super::*;
        use std::io::Write;
        use std::mem;

        #[test]
        fn connect() {
            let tcp_listener = std::net::TcpListener::bind("localhost:0").unwrap();
            let address = tcp_listener.local_addr().unwrap();

            let join_handle = std::thread::spawn(move || {
                let (_tcp_stream, _) = tcp_listener.accept().unwrap();
            });

            block_on(async {
                let tcp_stream = TcpStream::connect(address).await.unwrap();
                mem::forget(tcp_stream); // Test close separately
            });

            join_handle.join().unwrap();
        }

        #[test]
        fn fail_connect() {
            block_on(async {
                let tcp_stream = TcpStream::connect("localhost:0").await;
                tcp_stream.expect_err("Connection refused");
            });
        }

        #[test]
        fn read() {
            let tcp_listener = std::net::TcpListener::bind("localhost:0").unwrap();
            let address = tcp_listener.local_addr().unwrap();

            let join_handle = std::thread::spawn(move || {
                let (mut tcp_stream, _) = tcp_listener.accept().unwrap();

                let bytes_wrote = tcp_stream.write(b"hello").unwrap();
                assert_eq!(bytes_wrote, "hello".len());
            });

            block_on(async {
                let tcp_stream = TcpStream::connect(address).await.unwrap();

                let buffer = [0; 1024];
                let (_, buffer, bytes_read) = tcp_stream.read(buffer.into(), ..).await;
                assert_eq!(buffer[..bytes_read.unwrap()], b"hello"[..]);
            });

            join_handle.join().unwrap();
        }

        #[test]
        fn read_unchecked() {
            let tcp_listener = std::net::TcpListener::bind("localhost:0").unwrap();
            let address = tcp_listener.local_addr().unwrap();

            let join_handle = std::thread::spawn(move || {
                let (mut tcp_stream, _) = tcp_listener.accept().unwrap();

                let bytes_wrote = tcp_stream.write(b"hello").unwrap();
                assert_eq!(bytes_wrote, "hello".len());
            });

            block_on(async {
                let mut tcp_stream = TcpStream::connect(address).await.unwrap();

                let mut buffer = [0; 1024];
                let bytes_read = unsafe { tcp_stream.read_unchecked(&mut buffer) }.await;
                assert_eq!(buffer[..bytes_read.unwrap()], b"hello"[..]);
            });

            join_handle.join().unwrap();
        }

        #[test]
        fn write() {
            let tcp_listener = std::net::TcpListener::bind("localhost:0").unwrap();
            let address = tcp_listener.local_addr().unwrap();

            let join_handle = std::thread::spawn(move || {
                let (mut tcp_stream, _) = tcp_listener.accept().unwrap();

                let mut buffer = [0; 1024];
                let bytes_read = tcp_stream.read(&mut buffer).unwrap();
                assert_eq!(buffer[..bytes_read], b"hello"[..]);
            });

            block_on(async {
                let tcp_stream = TcpStream::connect(address).await.unwrap();

                let (_, _, bytes_wrote) = tcp_stream.write("hello".into(), ..).await;
                assert_eq!(bytes_wrote.unwrap(), "hello".len());
            });

            join_handle.join().unwrap();
        }

        #[test]
        fn write_unchecked() {
            let tcp_listener = std::net::TcpListener::bind("localhost:0").unwrap();
            let address = tcp_listener.local_addr().unwrap();

            let join_handle = std::thread::spawn(move || {
                let (mut tcp_stream, _) = tcp_listener.accept().unwrap();

                let mut buffer = [0; 1024];
                let bytes_read = tcp_stream.read(&mut buffer).unwrap();
                assert_eq!(buffer[..bytes_read], b"hello"[..]);
            });

            block_on(async {
                let mut tcp_stream = TcpStream::connect(address).await.unwrap();

                let bytes_wrote = unsafe { tcp_stream.write_unchecked(b"hello") }.await;
                assert_eq!(bytes_wrote.unwrap(), "hello".len());
            });

            join_handle.join().unwrap();
        }

        // TODO: read unchecked

        // FIXME: how to get send to fail??? (maybe: set_linger(None))
        // #[test]
        // fn fail_write() {
        //     let tcp_listener = std::net::TcpListener::bind("localhost:0").unwrap();
        //     let address = tcp_listener.local_addr().unwrap();
        //
        //     let join_handle = std::thread::spawn(move || {
        //         let (tcp_stream, _) = tcp_listener.accept().unwrap();
        //         // tcp_stream.shutdown(Shutdown::Both).unwrap();
        //         drop(tcp_stream);
        //     });
        //
        //     block_on(async {
        //         let mut tcp_stream = TcpStream::connect(address).await.unwrap();
        //         // join_handle.join().unwrap();
        //         let bytes_sent = unsafe { tcp_stream.send_unchecked(b"hello") }.await;
        //         assert_eq!(bytes_sent.unwrap(), 0);
        //     });
        //
        //     join_handle.join().unwrap();
        // }

        #[test]
        fn close() {
            let tcp_listener = std::net::TcpListener::bind("localhost:0").unwrap();
            let address = tcp_listener.local_addr().unwrap();

            let join_handle = std::thread::spawn(move || {
                let (mut tcp_stream, _) = tcp_listener.accept().unwrap();

                let mut buffer = [0; 1024];
                let bytes_read = tcp_stream.read(&mut buffer).unwrap();
                assert_eq!(bytes_read, 0);
            });

            block_on(async {
                let tcp_stream = TcpStream::connect(address).await.unwrap();

                let result = tcp_stream.close().await;
                assert!(result.is_ok());
            });

            join_handle.join().unwrap();
        }

        // FIXME: hard to test...
        // #[test]
        // fn implicit_close_connection() {
        //     let tcp_listener = std::net::TcpListener::bind("localhost:0").unwrap();
        //     let address = tcp_listener.local_addr().unwrap();
        //
        //     let join_handle = std::thread::spawn(move || {
        //         let (mut tcp_stream, _) = tcp_listener.accept().unwrap();
        //
        //         let mut buffer = [0; 1024];
        //         let bytes_read = tcp_stream.read(&mut buffer).unwrap();
        //         assert_eq!(bytes_read, 0);
        //     });
        //
        //     block_on(async {
        //         let tcp_stream = TcpStream::connect(address).await.unwrap();
        //         drop(tcp_stream); // FIXME: this spawns a task
        //                           // but doesn't get a chance to run...
        //
        //         // so I trick it:
        //         spawn(async {}).await.unwrap();
        //     });
        //
        //     join_handle.join().unwrap();
        // }
    }

    mod tcp_listener {
        use super::*;

        #[test]
        fn temp() {
            // let mut tcp_stream = std::net::TcpStream::connect(address).unwrap();
            // let mut buffer = [0; 1024];
            // let bytes_read = tcp_stream.read(&mut buffer).unwrap();
            // assert_eq!(buffer[..bytes_read], b"hello"[..]);
        }
    }
}
