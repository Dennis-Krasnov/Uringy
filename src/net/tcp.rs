//! ...

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::{io, mem};

use crate::runtime;

/// ...
#[derive(Debug)]
pub struct Stream(RawFd);

impl Stream {
    /// ...
    pub fn connect(address: impl super::ToSocketAddrs) -> crate::IoResult<Self> {
        let address = address.to_socket_addrs()?.next().unwrap().to_string();

        // TODO: ensure runtime exists
        // TODO: take std::net::IpAddr (dns -> happy eyes)
        // TODO: do this manually: https://www.geeksforgeeks.org/tcp-server-client-implementation-in-c/
        // let sqe = io_uring::opcode::Connect::new().build(); // TODO: benchmark difference!
        let stream = std::net::TcpStream::connect(address).unwrap();
        let fd = stream.into_raw_fd();
        Ok(Stream(fd))
    }

    /// ...
    pub fn peer_addr(&self) -> crate::IoResult<SocketAddr> {
        let stream = unsafe { std::net::TcpStream::from_raw_fd(self.0) };
        let addr = stream.peer_addr()?;
        mem::forget(stream);
        Ok(addr)
    }

    /// ...
    pub fn local_addr(&self) -> crate::IoResult<SocketAddr> {
        let stream = unsafe { std::net::TcpStream::from_raw_fd(self.0) };
        let addr = stream.local_addr()?;
        mem::forget(stream);
        Ok(addr)
    }

    // TODO: shutdown
    // TODO: try_clone or split
    // TODO: set_read_timeout, set_write_timeout, read_timeout, write_timeout; impl w/ linked op cancel
    // TODO: peek
    // TODO: set_linger/linger

    /// ...
    /// Nagle's algorithm coalesces small packets... smaller than needed packets wasting bandwidth... doesn't work well with TCP_NODELAY.
    /// modern applications do buffering themselves...
    /// TCP_NODELAY is for a specific purpose; to disable the Nagle buffering algorithm.
    /// It should only be set for applications that send frequent small bursts of information without getting an immediate response,
    /// where timely delivery of data is required (the canonical example is mouse movements).
    pub fn set_nodelay(&self, nodelay: bool) -> crate::IoResult<()> {
        let stream = unsafe { std::net::TcpStream::from_raw_fd(self.0) };
        stream.set_nodelay(nodelay)?;
        mem::forget(stream);
        Ok(())
    }

    /// ...
    pub fn nodelay(&self) -> crate::IoResult<bool> {
        let stream = unsafe { std::net::TcpStream::from_raw_fd(self.0) };
        let nodelay = stream.nodelay()?;
        mem::forget(stream);
        Ok(nodelay)
    }

    /// ...
    pub fn set_ttl(&self, ttl: u32) -> crate::IoResult<()> {
        let stream = unsafe { std::net::TcpStream::from_raw_fd(self.0) };
        stream.set_ttl(ttl)?;
        mem::forget(stream);
        Ok(())
    }

    /// ...
    pub fn ttl(&self) -> crate::IoResult<u32> {
        let stream = unsafe { std::net::TcpStream::from_raw_fd(self.0) };
        let ttl = stream.ttl()?;
        mem::forget(stream);
        Ok(ttl)
    }

    // TODO: take_error SO_ERROR
}

impl Drop for Stream {
    fn drop(&mut self) {
        let fd = io_uring::types::Fd(self.0);
        let sqe = io_uring::opcode::Close::new(fd).build();
        let _ = runtime::syscall(sqe);
    }
}

// TODO: implement on top of recv for reuse with OwnedReadHalf
impl io::Read for Stream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let fd = io_uring::types::Fd(self.0);
        let sqe = io_uring::opcode::Recv::new(fd, buffer.as_mut_ptr(), buffer.len() as u32).build();
        let bytes_read = runtime::syscall(sqe)?;
        Ok(bytes_read as usize)
    }
}

impl io::Write for Stream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let fd = io_uring::types::Fd(self.0);
        let sqe = io_uring::opcode::Send::new(fd, buffer.as_ptr(), buffer.len() as u32).build();
        let bytes_wrote = runtime::syscall(sqe)?;
        Ok(bytes_wrote as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// ...
#[derive(Debug)]
pub struct Listener(RawFd);

impl Listener {
    /// ...
    pub fn bind(address: impl super::ToSocketAddrs) -> crate::IoResult<Self> {
        // FIXME
        let address = address.to_socket_addrs()?.next().unwrap().to_string();
        let listener = std::net::TcpListener::bind(address)?;
        let fd = listener.as_raw_fd();
        mem::forget(listener);

        Ok(Listener(fd))
    }

    /// ...
    pub fn accept(&self) -> crate::IoResult<(Stream, SocketAddr)> {
        let fd = io_uring::types::Fd(self.0);
        let mut storage: libc::sockaddr_storage = unsafe { mem::zeroed() };
        let mut length = mem::size_of_val(&storage) as libc::socklen_t;
        let sqe = io_uring::opcode::Accept::new(fd, &mut storage as *mut _ as *mut _, &mut length)
            .flags(libc::SOCK_CLOEXEC)
            .build();
        let fd = runtime::syscall(sqe)?;
        let stream = Stream(RawFd::from(fd as i32));
        let addr = sockaddr_to_addr(&storage, length as usize)?;

        Ok((stream, addr))
    }

    // TODO: incoming, into_incoming

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
            let (server_addr_handle, server_addr) = crate::sync::channel::unbounded();
            let (client_addr_handle, client_addr) = crate::sync::channel::unbounded();

            spawn(move || {
                let listener = Listener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
                let server_addr = listener.local_addr().unwrap();
                server_addr_handle.send(server_addr).unwrap();

                let (mut tcp, address) = listener.accept().unwrap();
                client_addr_handle.send(address).unwrap();
                tcp.write_all(b"hello").unwrap();
            });

            let server_addr = server_addr.recv().unwrap();
            let mut tcp = Stream::connect((Ipv4Addr::LOCALHOST, server_addr.port())).unwrap();
            let client_addr = client_addr.recv().unwrap();
            assert_eq!(tcp.peer_addr().unwrap().port(), server_addr.port());
            assert_eq!(tcp.local_addr().unwrap(), client_addr);

            let mut buffer = vec![0; 1024];
            let bytes_read = tcp.read(&mut buffer).unwrap();
            assert_eq!(&buffer[..bytes_read], b"hello");
        })
        .unwrap();
    }
}

// pub fn setsockopt<T>(
//     sock: &Socket,
//     level: c_int,
//     option_name: c_int,
//     option_value: T,
// ) -> io::Result<()> {
//     unsafe {
//         cvt(c::setsockopt(
//             sock.as_raw(),
//             level,
//             option_name,
//             &option_value as *const T as *const _,
//             mem::size_of::<T>() as c::socklen_t,
//         ))?;
//         Ok(())
//     }
// }

// pub fn getsockopt<T: Copy>(sock: &Socket, level: c_int, option_name: c_int) -> io::Result<T> {
//     unsafe {
//         let mut option_value: T = mem::zeroed();
//         let mut option_len = mem::size_of::<T>() as c::socklen_t;
//         cvt(c::getsockopt(
//             sock.as_raw(),
//             level,
//             option_name,
//             &mut option_value as *mut T as *mut _,
//             &mut option_len,
//         ))?;
//         Ok(option_value)
//     }
// }

// pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
//     setsockopt(&self.inner, c::IPPROTO_IP, c::IP_TTL, ttl as c_int)
// }
//
// pub fn ttl(&self) -> io::Result<u32> {
//     let raw: c_int = getsockopt(&self.inner, c::IPPROTO_IP, c::IP_TTL)?;
//     Ok(raw as u32)
// }
