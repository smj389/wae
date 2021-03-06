use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

use super::get_addr_info::get_addr_info;

pub trait ToSocketAddrs {
    fn to_socket_addrs(&self) -> sealed::ToSocketAddrs;
}

impl ToSocketAddrs for SocketAddr {
    fn to_socket_addrs(&self) -> sealed::ToSocketAddrs {
        sealed::ToSocketAddrs::Immediate { addr: *self }
    }
}

macro_rules! impl_into {
    ($ty:ty) => {
        impl ToSocketAddrs for $ty {
            fn to_socket_addrs(&self) -> sealed::ToSocketAddrs {
                sealed::ToSocketAddrs::Immediate {
                    addr: (*self).into(),
                }
            }
        }
    };
}

impl_into!(SocketAddrV4);
impl_into!(SocketAddrV6);
impl_into!((IpAddr, u16));
impl_into!((Ipv4Addr, u16));
impl_into!((Ipv6Addr, u16));

impl<'a> ToSocketAddrs for &'a [SocketAddr] {
    fn to_socket_addrs(&self) -> sealed::ToSocketAddrs<'a> {
        sealed::ToSocketAddrs::Slice { addrs: self }
    }
}

impl ToSocketAddrs for str {
    fn to_socket_addrs(&self) -> sealed::ToSocketAddrs {
        match self.parse() {
            Ok(addr) => sealed::ToSocketAddrs::Immediate { addr },
            Err(_) => sealed::ToSocketAddrs::Future {
                future: get_addr_info(self, None),
            },
        }
    }
}

impl ToSocketAddrs for String {
    fn to_socket_addrs(&self) -> sealed::ToSocketAddrs {
        <str as ToSocketAddrs>::to_socket_addrs(self)
    }
}

impl ToSocketAddrs for (&str, u16) {
    fn to_socket_addrs(&self) -> sealed::ToSocketAddrs {
        match self.0.parse() {
            Ok(ip) => sealed::ToSocketAddrs::Immediate {
                addr: SocketAddr::new(ip, self.1),
            },
            Err(_) => sealed::ToSocketAddrs::Future {
                future: get_addr_info(self.0, Some(self.1)),
            },
        }
    }
}

impl ToSocketAddrs for (String, u16) {
    fn to_socket_addrs(&self) -> sealed::ToSocketAddrs {
        match self.0.parse() {
            Ok(ip) => sealed::ToSocketAddrs::Immediate {
                addr: SocketAddr::new(ip, self.1),
            },
            Err(_) => sealed::ToSocketAddrs::Future {
                future: get_addr_info(&self.0, Some(self.1)),
            },
        }
    }
}

impl<T: ToSocketAddrs + ?Sized> ToSocketAddrs for &T {
    fn to_socket_addrs(&self) -> sealed::ToSocketAddrs {
        (&**self).to_socket_addrs()
    }
}

mod sealed {
    use std::{
        future::Future,
        io,
        iter::{self, Copied, Once},
        net::SocketAddr,
        pin::Pin,
        slice::Iter,
        task::{Context, Poll},
    };

    use pin_project_lite::pin_project;

    use crate::net::socket_addr::get_addr_info::{GetAddrInfoFuture, GetAddrInfoIter};

    pin_project! {
        #[project = ToSocketAddrsProj]
        pub enum ToSocketAddrs<'a> {
            Immediate { addr: SocketAddr },
            Slice { addrs: &'a [SocketAddr] },
            Future { #[pin] future: GetAddrInfoFuture },
        }
    }

    impl<'a> Future for ToSocketAddrs<'a> {
        type Output = io::Result<ToSocketAddrsIter<'a>>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
            match self.project() {
                ToSocketAddrsProj::Immediate { addr } => {
                    Poll::Ready(Ok(ToSocketAddrsIter::Immediate(iter::once(*addr))))
                }
                ToSocketAddrsProj::Slice { addrs } => {
                    Poll::Ready(Ok(ToSocketAddrsIter::Slice(addrs.iter().copied())))
                }
                ToSocketAddrsProj::Future { future } => match future.poll(cx) {
                    Poll::Ready(Ok(iter)) => Poll::Ready(Ok(ToSocketAddrsIter::Future(iter))),
                    Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
                    Poll::Pending => Poll::Pending,
                },
            }
        }
    }

    pub enum ToSocketAddrsIter<'a> {
        Immediate(Once<SocketAddr>),
        Slice(Copied<Iter<'a, SocketAddr>>),
        Future(GetAddrInfoIter),
    }

    impl Iterator for ToSocketAddrsIter<'_> {
        type Item = SocketAddr;

        fn next(&mut self) -> Option<Self::Item> {
            match self {
                ToSocketAddrsIter::Immediate(iter) => iter.next(),
                ToSocketAddrsIter::Slice(iter) => iter.next(),
                ToSocketAddrsIter::Future(iter) => iter.next(),
            }
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            match self {
                ToSocketAddrsIter::Immediate(iter) => iter.size_hint(),
                ToSocketAddrsIter::Slice(iter) => iter.size_hint(),
                ToSocketAddrsIter::Future(iter) => iter.size_hint(),
            }
        }
    }
}
