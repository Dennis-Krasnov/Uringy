//! ...

use crate::circular_buffer::Uninit;
use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::rc::Rc;
use std::{io, mem};

use crate::{runtime, IoResult};

/// ...
pub fn connect(address: impl super::ToSocketAddrs) -> IoResult<(WriteHalf, ReadHalf)> {
    let address = address.to_socket_addrs()?.next().unwrap().to_string();

    // TODO: ensure runtime exists
    // TODO: take std::net::IpAddr (dns -> happy eyes)
    // TODO: do this manually: https://www.geeksforgeeks.org/tcp-server-client-implementation-in-c/
    // let sqe = io_uring::opcode::Connect::new().build(); // TODO: benchmark difference!
    let stream = std::net::TcpStream::connect(address).unwrap();
    let fd = stream.into_raw_fd();

    let state = Rc::new(RefCell::new(StreamState { fd }));

    Ok((WriteHalf(state.clone()), ReadHalf(state)))
}

/// ...
pub struct WriteHalf(Rc<RefCell<StreamState>>);

impl Write for WriteHalf {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let fd = io_uring::types::Fd(self.0.borrow().fd);
        let sqe = io_uring::opcode::Send::new(fd, buffer.as_ptr(), buffer.len() as u32).build();
        let bytes_wrote = runtime::syscall(sqe)?;
        Ok(bytes_wrote as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// ...
pub struct ReadHalf(Rc<RefCell<StreamState>>);

impl Read for ReadHalf {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let fd = io_uring::types::Fd(self.0.borrow().fd);
        let sqe = io_uring::opcode::Recv::new(fd, buffer.as_mut_ptr(), buffer.len() as u32).build();
        let bytes_read = runtime::syscall(sqe)?;
        Ok(bytes_read as usize)
    }
}

#[derive(Debug)]
struct StreamState {
    fd: RawFd,
}

/// ...
#[derive(Debug)]
pub struct Listener(RawFd);

impl Listener {
    /// ...
    pub fn bind(address: impl super::ToSocketAddrs) -> crate::IoResult<Self> {
        // FIXME non-blocking
        let address = address.to_socket_addrs()?.next().unwrap().to_string();
        let listener = std::net::TcpListener::bind(address)?;
        let fd = listener.as_raw_fd();
        mem::forget(listener);

        Ok(Listener(fd))
    }

    /// ...
    pub fn accept(&self) -> crate::IoResult<((WriteHalf, ReadHalf), SocketAddr)> {
        let fd = io_uring::types::Fd(self.0);
        let mut storage: libc::sockaddr_storage = unsafe { mem::zeroed() };
        let mut length = mem::size_of_val(&storage) as libc::socklen_t;
        let sqe = io_uring::opcode::Accept::new(fd, &mut storage as *mut _ as *mut _, &mut length)
            .flags(libc::SOCK_CLOEXEC)
            .build();
        let fd = runtime::syscall(sqe)?;

        let fd = RawFd::from(fd as i32);
        let state = Rc::new(RefCell::new(StreamState { fd }));
        let stream = (WriteHalf(state.clone()), ReadHalf(state));

        let addr = sockaddr_to_addr(&storage, length as usize)?;

        Ok((stream, addr))
    }

    // TODO: incoming, into_incoming
    /// not the same as std library! can return None...
    pub fn into_incoming(self) -> IntoIncoming {
        IntoIncoming(self)
    }

    /// ...
    pub fn local_addr(&self) -> crate::IoResult<SocketAddr> {
        let listener = unsafe { std::net::TcpListener::from_raw_fd(self.0) };
        let addr = listener.local_addr()?;
        mem::forget(listener);
        Ok(addr)
    }

    /// ...
    pub fn set_ttl(&self, ttl: u32) -> crate::IoResult<()> {
        let listener = unsafe { std::net::TcpListener::from_raw_fd(self.0) };
        listener.set_ttl(ttl)?;
        mem::forget(listener);
        Ok(())
    }

    /// ...
    pub fn ttl(&self) -> crate::IoResult<u32> {
        let listener = unsafe { std::net::TcpListener::from_raw_fd(self.0) };
        let ttl = listener.ttl()?;
        mem::forget(listener);
        Ok(ttl)
    }

    // TODO: take_error SO_ERROR
}

impl Drop for Listener {
    fn drop(&mut self) {
        let fd = io_uring::types::Fd(self.0);
        let sqe = io_uring::opcode::Close::new(fd).build();
        let _ = runtime::syscall(sqe);
    }
}

/// ...
pub struct IntoIncoming(Listener);

impl Iterator for IntoIncoming {
    type Item = (WriteHalf, ReadHalf);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.accept().map(|(s, _)| s).ok()
    }
}

fn sockaddr_to_addr(storage: &libc::sockaddr_storage, length: usize) -> io::Result<SocketAddr> {
    match storage.ss_family as libc::c_int {
        libc::AF_INET => {
            assert!(length >= mem::size_of::<libc::sockaddr_in>());
            let addr = unsafe { *(storage as *const _ as *const libc::sockaddr_in) };

            Ok(SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::from(addr.sin_addr.s_addr.to_ne_bytes()),
                u16::from_be(addr.sin_port),
            )))
        }
        libc::AF_INET6 => {
            assert!(length >= mem::size_of::<libc::sockaddr_in6>());
            let addr = unsafe { *(storage as *const _ as *const libc::sockaddr_in6) };

            Ok(SocketAddr::V6(SocketAddrV6::new(
                Ipv6Addr::from(addr.sin6_addr.s6_addr),
                u16::from_be(addr.sin6_port),
                addr.sin6_flowinfo,
                addr.sin6_scope_id,
            )))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid argument",
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::Ipv4Addr;

    use crate::runtime::{spawn, start};

    use super::*;

    #[test]
    fn smoke() {
        start(|| {
            let listener = Listener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
            let server_addr = listener.local_addr().unwrap();

            let (client_addr_handle, _client_addr) = crate::sync::channel::unbounded();

            spawn(move || {
                let ((mut w, mut r), address) = listener.accept().unwrap();
                client_addr_handle.send(address).unwrap();

                let mut buffer = vec![0; 1024];
                let bytes_read = r.read(&mut buffer).unwrap();
                w.write_all(&buffer[..bytes_read]).unwrap();
            });

            let (mut w, mut r) = connect((Ipv4Addr::LOCALHOST, server_addr.port())).unwrap();
            // assert_eq!(w.addr(), server_addr);
            // assert_eq!(r.addr(), client_addr.recv().unwrap());

            w.write_all(b"hello").unwrap();

            let mut buffer = vec![0; 1024];
            let bytes_read = r.read(&mut buffer).unwrap();
            assert_eq!(&buffer[..bytes_read], b"hello");
        })
        .unwrap();
    }

    // #[test]
    // // #[ignore = "takes 16s to run in release mode"]
    // fn cleans_up_after_itself() {
    //     start(|| {
    //         // enough to hit OS limits
    //         for _ in 0..1_000_000 {
    //             // FIXME: this hits the wrong os limits: called `Result::unwrap()` on an `Err` value: Original(Os { code: 98, kind: AddrInUse, message: "Address already in use" })
    //             //  need to set flag on tcp stream to prevent wait state
    //             let listener = Listener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
    //             let port = listener.local_addr().unwrap().port();
    //             let server = spawn(move || drop(listener.accept().unwrap().0));
    //             drop(connect((Ipv4Addr::LOCALHOST, port)).unwrap());
    //             server.join().unwrap();
    //         }
    //     })
    //     .unwrap();
    // }
}
