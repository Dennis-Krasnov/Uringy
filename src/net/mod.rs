//! ...

use crate::IoResult;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::option;

pub mod tcp;

/// ...
pub trait ToSocketAddrs {
    /// ...
    type Iter: Iterator<Item = SocketAddr>;

    /// ...
    fn to_socket_addrs(&self) -> IoResult<Self::Iter>;
}

// FIXME: I don't want this... I want to manually resolve.
// impl<T: std::net::ToSocketAddrs> ToSocketAddrs for T {
//     type Iter = T::Iter;
//
//     fn to_socket_addrs(&self) -> io::Result<option::IntoIter<SocketAddr>> {
//         Ok(Some(*self).into_iter())
//     }
// }

impl ToSocketAddrs for SocketAddr {
    type Iter = option::IntoIter<SocketAddr>;

    fn to_socket_addrs(&self) -> IoResult<Self::Iter> {
        Ok(Some(*self).into_iter())
    }
}

impl ToSocketAddrs for SocketAddrV4 {
    type Iter = option::IntoIter<SocketAddr>;

    fn to_socket_addrs(&self) -> IoResult<Self::Iter> {
        SocketAddr::V4(*self).to_socket_addrs()
    }
}
//
// impl ToSocketAddrs for SocketAddrV6 {
//     type Iter = option::IntoIter<SocketAddr>;
//
//     fn to_socket_addrs(&self) -> io::Result<option::IntoIter<SocketAddr>> {
//         SocketAddr::V6(*self).to_socket_addrs()
//     }
// }
//
// impl ToSocketAddrs for (IpAddr, u16) {
//     type Iter = option::IntoIter<SocketAddr>;
//
//     fn to_socket_addrs(&self) -> io::Result<option::IntoIter<SocketAddr>> {
//         let (ip, port) = *self;
//         match ip {
//             IpAddr::V4(addr) => (addr, port).to_socket_addrs(),
//             IpAddr::V6(addr) => (addr, port).to_socket_addrs(),
//         }
//     }
// }
//
impl ToSocketAddrs for (Ipv4Addr, u16) {
    type Iter = option::IntoIter<SocketAddr>;

    fn to_socket_addrs(&self) -> IoResult<Self::Iter> {
        let (ip, port) = *self;
        SocketAddrV4::new(ip, port).to_socket_addrs()
    }
}
//
// impl ToSocketAddrs for (Ipv6Addr, u16) {
//     type Iter = option::IntoIter<SocketAddr>;
//
//     fn to_socket_addrs(&self) -> io::Result<option::IntoIter<SocketAddr>> {
//         let (ip, port) = *self;
//         SocketAddrV6::new(ip, port, 0, 0).to_socket_addrs()
//     }
// }
//
// impl ToSocketAddrs for (&str, u16) {
//     // type Iter = vec::IntoIter<SocketAddr>;
//     type Iter = sync::channel::Receiver<SocketAddr>;
//
//     fn to_socket_addrs(&self) -> io::Result<sync::channel::Receiver<SocketAddr>> {
//         let (host, port) = *self;
//         let (tx, rx) = sync::channel::unbounded();
//
//         if let Ok(addr) = host.parse() {
//             let addr = SocketAddrV4::new(addr, port);
//             tx.send(SocketAddr::V4(addr)).unwrap();
//             return Ok(rx);
//             // return Ok(vec![SocketAddr::V4(addr)].into_iter());
//         }
//
//         if let Ok(addr) = host.parse() {
//             let addr = SocketAddrV6::new(addr, port, 0, 0);
//             tx.send(SocketAddr::V6(addr)).unwrap();
//             return Ok(rx);
//             // return Ok(vec![SocketAddr::V6(addr)].into_iter());
//         }
//
//         spawn(move || {
//             drop(tx);
//             // TODO: do DNS stuff, send to tx
//         });
//
//         Ok(rx)
//
//         // // TODO: DNS returns a read channel handle (implements iterator) (continues to do stuff in background)
//         // let addresses: Vec<_> = dns::dig_short(host)?
//         //     .into_iter()
//         //     .map(|ip| SocketAddr::new(ip, port))
//         //     .collect();
//         // Ok(addresses.into_iter())
//     }
// }
//
// impl ToSocketAddrs for (String, u16) {
//     // type Iter = vec::IntoIter<SocketAddr>;
//     type Iter = sync::channel::Receiver<SocketAddr>;
//
//     fn to_socket_addrs(&self) -> io::Result<sync::channel::Receiver<SocketAddr>> {
//         (&*self.0, self.1).to_socket_addrs()
//     }
// }
//
// // accepts strings like 'localhost:12345'
// impl ToSocketAddrs for str {
//     // type Iter = vec::IntoIter<SocketAddr>;
//     type Iter = sync::channel::Receiver<SocketAddr>;
//
//     fn to_socket_addrs(&self) -> io::Result<sync::channel::Receiver<SocketAddr>> {
//         if let Ok(addr) = self.parse() {
//             let (tx, rx) = sync::channel::unbounded();
//             tx.send(addr).unwrap();
//             return Ok(rx);
//             // return Ok(vec![addr].into_iter());
//         }
//
//         let Some((host, port)) = self.rsplit_once(':') else {
//             return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid socket address"));
//         };
//         let Ok(port) = port.parse() else {
//             return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid port value"));
//         };
//         (host, port).to_socket_addrs()
//     }
// }
//
// impl<'a> ToSocketAddrs for &'a [SocketAddr] {
//     type Iter = iter::Cloned<slice::Iter<'a, SocketAddr>>;
//
//     fn to_socket_addrs(&self) -> io::Result<Self::Iter> {
//         Ok(self.iter().cloned())
//     }
// }
//
// impl<T: ToSocketAddrs + ?Sized> ToSocketAddrs for &T {
//     type Iter = T::Iter;
//
//     fn to_socket_addrs(&self) -> io::Result<T::Iter> {
//         (**self).to_socket_addrs()
//     }
// }
//
// impl ToSocketAddrs for String {
//     type Iter = sync::channel::Receiver<SocketAddr>;
//
//     fn to_socket_addrs(&self) -> io::Result<sync::channel::Receiver<SocketAddr>> {
//         (&**self).to_socket_addrs()
//     }
// }
