#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/lapin-futures-tls-api/0.6.0/")]

//! lapin-futures-tls-api
//!
//! This library offers a nice integration of `tls-api` with the `lapin-futures` library.
//! It uses `amq-protocol` URI parsing feature and adds a `connect` method to `AMQPUri`
//! which will provide you with a `lapin_futures::client::Client` wrapped in a `Future`.
//!
//! It autodetects whether you're using `amqp` or `amqps` and opens either a raw `TcpStream`
//! or a `TlsStream` using `tls-api` as the SSL api.
//!
//! ## Connecting and opening a channel
//!
//! ```rust,no_run
//! extern crate env_logger;
//! extern crate futures;
//! extern crate lapin_futures_tls_api;
//! extern crate tls_api_stub;
//! extern crate tokio_core;
//!
//! use lapin_futures_tls_api::lapin;
//!
//! use futures::future::Future;
//! use lapin::channel::ConfirmSelectOptions;
//! use lapin_futures_tls_api::AMQPConnectionExt;
//! use tokio_core::reactor::Core;
//!
//! fn main() {
//!     env_logger::init();
//!
//!     let mut core = Core::new().unwrap();
//!     let handle   = core.handle();
//!
//!     core.run(
//!         "amqps://user:pass@host/vhost?heartbeat=10".connect::<tls_api_stub::TlsConnector, _>(handle, |_| ()).and_then(|client| {
//!             println!("Connected!");
//!             client.create_confirm_channel(ConfirmSelectOptions::default())
//!         }).and_then(|channel| {
//!             println!("Closing channel.");
//!             channel.close(200, "Bye")
//!         })
//!     ).unwrap();
//! }
//! ```

extern crate amq_protocol;
extern crate bytes;
extern crate futures;
extern crate lapin_futures;
extern crate tls_api;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_tls_api;
extern crate trust_dns_resolver;

/// Reexport of the `lapin_futures` crate
pub mod lapin;
/// Reexport of the `uri` module from the `amq_protocol` crate
pub mod uri;

use std::io::{self, Read, Write};
use std::net::SocketAddr;

use bytes::{Buf, BufMut};
use futures::future::Future;
use futures::Poll;
use tls_api::{TlsConnector, TlsConnectorBuilder};
use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_tls_api::TlsStream;
use trust_dns_resolver::ResolverFuture;

use lapin::client::ConnectionOptions;
use uri::{AMQPQueryString, AMQPScheme, AMQPUri, AMQPUserInfo};

/// Represents either a raw `TcpStream` or a `TlsStream` backend by `tokio-tls-api`.
/// The `TlsStream` is wrapped in a `Box` to keep the enum footprint minimal.
pub enum AMQPStream {
    /// The raw `TcpStream` used for basic AMQP connections.
    Raw(TcpStream),
    /// The `TlsStream` used for AMQPs connections.
    Tls(Box<TlsStream<TcpStream>>),
}

/// Add a connect method providing a `lapin_futures::client::Client` wrapped in a `Future`.
pub trait AMQPConnectionExt {
    /// Method providing a `lapin_futures::client::Client` wrapped in a `Future`
    /// using a `tokio_code::reactor::Handle`.
    fn connect<C: TlsConnector + 'static, F: FnOnce(io::Error) + 'static>(&self, handle: Handle, heartbeat_error_handler: F) -> Box<Future<Item = lapin::client::Client<AMQPStream>, Error = io::Error> + 'static>;
}

impl AMQPConnectionExt for AMQPUri {
    fn connect<C: TlsConnector + 'static, F: FnOnce(io::Error) + 'static>(&self, handle: Handle, heartbeat_error_handler: F) -> Box<Future<Item = lapin::client::Client<AMQPStream>, Error = io::Error> + 'static> {
        let userinfo = self.authority.userinfo.clone();
        let vhost    = self.vhost.clone();
        let query    = self.query.clone();
        let stream   = match self.scheme {
            AMQPScheme::AMQP  => AMQPStream::raw(&handle, self.authority.host.clone(), self.authority.port),
            AMQPScheme::AMQPS => AMQPStream::tls::<C>(&handle, self.authority.host.clone(), self.authority.port),
        };
        let handle   = handle.clone();

        Box::new(stream.and_then(move |stream| connect_stream(stream, handle, userinfo, vhost, &query, heartbeat_error_handler)))
    }
}

impl AMQPConnectionExt for str {
    fn connect<C: TlsConnector + 'static, F: FnOnce(io::Error) + 'static>(&self, handle: Handle, heartbeat_error_handler: F) -> Box<Future<Item = lapin::client::Client<AMQPStream>, Error = io::Error> + 'static> {
        match self.parse::<AMQPUri>() {
            Ok(uri)  => uri.connect::<C, F>(handle, heartbeat_error_handler),
            Err(err) => Box::new(futures::future::err(io::Error::new(io::ErrorKind::Other, err))),
        }
    }
}

impl AMQPStream {
    fn raw(handle: &Handle, host: String, port: u16) -> Box<Future<Item = Self, Error = io::Error> + 'static> {
        Box::new(open_tcp_stream(handle, host, port).map(AMQPStream::Raw))
    }

    fn tls<C: TlsConnector + 'static>(handle: &Handle, host: String, port: u16) -> Box<Future<Item = Self, Error = io::Error> + 'static> {
        Box::new(
            open_tcp_stream(handle, host.clone(), port).join(
                futures::future::result(C::builder().and_then(TlsConnectorBuilder::build).map_err(From::from))
            ).and_then(move |(stream, connector)| {
                tokio_tls_api::connect_async(&connector, &host, stream).map_err(From::from).map(Box::new).map(AMQPStream::Tls)
            })
        )
    }
}

impl Read for AMQPStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match *self {
            AMQPStream::Raw(ref mut raw) => raw.read(buf),
            AMQPStream::Tls(ref mut tls) => tls.read(buf),
        }
    }
}

impl AsyncRead for AMQPStream {
    unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [u8]) -> bool {
        match *self {
            AMQPStream::Raw(ref raw) => raw.prepare_uninitialized_buffer(buf),
            AMQPStream::Tls(ref tls) => tls.prepare_uninitialized_buffer(buf),
        }
    }

    fn read_buf<B: BufMut>(&mut self, buf: &mut B) -> Poll<usize, io::Error> {
        match *self {
            AMQPStream::Raw(ref mut raw) => raw.read_buf(buf),
            AMQPStream::Tls(ref mut tls) => tls.read_buf(buf),
        }
    }
}

impl Write for AMQPStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match *self {
            AMQPStream::Raw(ref mut raw) => raw.write(buf),
            AMQPStream::Tls(ref mut tls) => tls.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match *self {
            AMQPStream::Raw(ref mut raw) => raw.flush(),
            AMQPStream::Tls(ref mut tls) => tls.flush(),
        }
    }
}

impl AsyncWrite for AMQPStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        match *self {
            AMQPStream::Raw(ref mut raw) => raw.shutdown(),
            AMQPStream::Tls(ref mut tls) => tls.shutdown(),
        }
    }

    fn write_buf<B: Buf>(&mut self, buf: &mut B) -> Poll<usize, io::Error> {
        match *self {
            AMQPStream::Raw(ref mut raw) => raw.write_buf(buf),
            AMQPStream::Tls(ref mut tls) => tls.write_buf(buf),
        }
    }
}

fn open_tcp_stream(handle: &Handle, host: String, port: u16) -> Box<Future<Item = TcpStream, Error = io::Error> + 'static> {
    let resolver = ResolverFuture::from_system_conf(handle).map_err(From::from);
    let host     = host.clone();
    let handle   = handle.clone();
    Box::new(
        futures::future::result(resolver).and_then(move |resolver| {
            resolver.lookup_ip(&host).map_err(From::from)
        }).and_then(|response| {
            response.iter().next().ok_or_else(|| io::Error::new(io::ErrorKind::AddrNotAvailable, "Couldn't resolve hostname"))
        }).and_then(move |ipaddr| {
            TcpStream::connect(&SocketAddr::new(ipaddr, port), &handle)
        })
    )
}

fn connect_stream<T: AsyncRead + AsyncWrite + Send + Sync + 'static, F: FnOnce(io::Error) + 'static>(stream: T, handle: Handle, credentials: AMQPUserInfo, vhost: String, query: &AMQPQueryString, heartbeat_error_handler: F) -> Box<Future<Item = lapin::client::Client<T>, Error = io::Error> + 'static> {
    let defaults = ConnectionOptions::default();
    Box::new(lapin::client::Client::connect(stream, &ConnectionOptions {
        username:  credentials.username,
        password:  credentials.password,
        vhost:     vhost,
        frame_max: query.frame_max.unwrap_or_else(|| defaults.frame_max),
        heartbeat: query.heartbeat.unwrap_or_else(|| defaults.heartbeat),
    }).map(move |(client, heartbeat_future_fn)| {
        let heartbeat_client = client.clone();
        handle.spawn(heartbeat_future_fn(&heartbeat_client).map_err(heartbeat_error_handler));
        client
    }))
}
