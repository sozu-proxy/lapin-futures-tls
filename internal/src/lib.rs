#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/lapin-futures-tls-internal/0.3.1/")]

//! lapin-futures-openssl
//!
//! This library offers a nice integration of `openssl` with the `lapin-futures` library.
//! It uses `amq-protocol` URI parsing feature and adds the `connect` and `connect_cancellable`
//! methods to `AMQPUri` which will provide you with a `lapin_futures::client::Client` and
//! optionally a `lapin_futures::client::HeartbeatHandle` wrapped in a `Future`.
//!
//! It autodetects whether you're using `amqp` or `amqps` and opens either a raw `TcpStream`
//! or a `TlsStream`.
//!
//! ## Connecting and opening a channel
//!
//! ```rust,no_run
//! extern crate env_logger;
//! extern crate futures;
//! extern crate lapin_futures_tls_internal;
//! extern crate native_tls;
//! extern crate tokio;
//! extern crate tokio_tls;
//! 
//! use lapin_futures_tls_internal::lapin;
//! 
//! use futures::future::Future;
//! use lapin::channel::ConfirmSelectOptions;
//! use lapin_futures_tls_internal::AMQPConnectionTlsExt;
//! use tokio_tls::TlsConnector;
//! 
//! use std::io;
//! 
//! fn main() {
//!     env_logger::init();
//! 
//!     tokio::run(
//!         "amqps://user:pass@host/vhost?heartbeat=10".connect_cancellable(|err| {
//!             eprintln!("heartbeat error: {:?}", err);
//!         }, |host, stream| {
//!             Box::new(futures::future::result(native_tls::TlsConnector::builder().build().map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to create connector"))).and_then(move |connector| {
//!                 TlsConnector::from(connector).connect(&host, stream).map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to connect")).map(Box::new)
//!             }))
//!         }).and_then(|(client, heartbeat_handle)| {
//!             println!("Connected!");
//!             client.create_confirm_channel(ConfirmSelectOptions::default()).map(|channel| (channel, heartbeat_handle)).and_then(|(channel, heartbeat_handle)| {
//!                 println!("Stopping heartbeat.");
//!                 heartbeat_handle.stop();
//!                 println!("Closing channel.");
//!                 channel.close(200, "Bye")
//!             }).map_err(From::from)
//!         }).map_err(|err| {
//!             eprintln!("amqp error: {:?}", err);
//!         })
//!     );
//! }
//! ```

extern crate bytes;
extern crate failure;
extern crate futures;
extern crate lapin_futures;
extern crate tokio_executor;
extern crate tokio_io;
extern crate tokio_tcp;
extern crate trust_dns_resolver;

/// The type errors that can be returned in this crate.
pub mod error;
/// Reexport of the `lapin_futures` crate
pub mod lapin;
/// Reexport of the `uri` module from the `amq_protocol` crate
pub mod uri;

/// Reexport of `TcpStream`
pub use tokio_tcp::TcpStream;

use std::io::{self, Read, Write};
use std::net::SocketAddr;

use bytes::{Buf, BufMut};
use futures::future::Future;
use futures::Poll;
use tokio_io::{AsyncRead, AsyncWrite};
use trust_dns_resolver::AsyncResolver;

use error::{Error, ErrorKind};
use lapin::client::ConnectionOptions;
use uri::{AMQPScheme, AMQPUri};

/// Represents either a raw `TcpStream` or a `TlsStream`.
/// The `TlsStream` is wrapped in a `Box` to keep the enum footprint minimal.
pub enum AMQPStream<TlsStream: AsyncRead + AsyncWrite + Send + 'static> {
    /// The raw `TcpStream` used for basic AMQP connections.
    Raw(TcpStream),
    /// The `TlsStream` used for AMQPs connections.
    Tls(Box<TlsStream>),
}

/// Add a connect method providing a `lapin_futures::client::Client` wrapped in a `Future`.
pub trait AMQPConnectionTlsExt<TlsStream: AsyncRead + AsyncWrite + Send + Sync + 'static, F: FnOnce(Error) + Send + 'static> {
    /// Method providing a `lapin_futures::client::Client` wrapped in a `Future`
    fn connect<Connector: FnOnce(String, TcpStream) -> Box<Future<Item = Box<TlsStream>, Error = io::Error> + Send + 'static> + Send + 'static>(self, heartbeat_error_handler: F, connector: Connector) -> Box<Future<Item = lapin::client::Client<AMQPStream<TlsStream>>, Error = Error> + Send + 'static>;
    /// Method providing a `lapin_futures::client::Client` and `lapin_futures::client::HeartbeatHandle` wrapped in a `Future`
    fn connect_cancellable<Connector: FnOnce(String, TcpStream) -> Box<Future<Item = Box<TlsStream>, Error = io::Error> + Send + 'static> + Send + 'static>(self, heartbeat_error_handler: F, connector: Connector) -> Box<Future<Item = (lapin::client::Client<AMQPStream<TlsStream>>, lapin::client::HeartbeatHandle), Error = Error> + Send + 'static>;
}

impl<TlsStream: AsyncRead + AsyncWrite + Send + Sync + 'static, F: FnOnce(Error) + Send + 'static> AMQPConnectionTlsExt<TlsStream, F> for AMQPUri {
    fn connect<Connector: FnOnce(String, TcpStream) -> Box<Future<Item = Box<TlsStream>, Error = io::Error> + Send + 'static> + Send + 'static>(self, heartbeat_error_handler: F, connector: Connector) -> Box<Future<Item = lapin::client::Client<AMQPStream<TlsStream>>, Error = Error> + Send + 'static> {
        Box::new(AMQPStream::from_amqp_uri(&self, connector).and_then(move |stream| connect_stream(stream, self, heartbeat_error_handler, false)).map(|(client, _)| client))
    }

    fn connect_cancellable<Connector: FnOnce(String, TcpStream) -> Box<Future<Item = Box<TlsStream>, Error = io::Error> + Send + 'static> + Send + 'static>(self, heartbeat_error_handler: F, connector: Connector) -> Box<Future<Item = (lapin::client::Client<AMQPStream<TlsStream>>, lapin::client::HeartbeatHandle), Error = Error> + Send + 'static> {
        Box::new(AMQPStream::from_amqp_uri(&self, connector).and_then(move |stream| connect_stream(stream, self, heartbeat_error_handler, true)).map(|(client, heartbeat_handle)| (client, heartbeat_handle.unwrap())))
    }
}

impl<'a, TlsStream: AsyncRead + AsyncWrite + Send + Sync + 'static, F: FnOnce(Error) + Send + 'static> AMQPConnectionTlsExt<TlsStream, F> for &'a str {
    fn connect<Connector: FnOnce(String, TcpStream) -> Box<Future<Item = Box<TlsStream>, Error = io::Error> + Send + 'static> + Send + 'static>(self, heartbeat_error_handler: F, connector: Connector) -> Box<Future<Item = lapin::client::Client<AMQPStream<TlsStream>>, Error = Error> + Send + 'static> {
        match self.parse::<AMQPUri>() {
            Ok(uri)  => uri.connect(heartbeat_error_handler, connector),
            Err(err) => Box::new(futures::future::err(ErrorKind::UriParsingError(err).into())),
        }
    }

    fn connect_cancellable<Connector: FnOnce(String, TcpStream) -> Box<Future<Item = Box<TlsStream>, Error = io::Error> + Send + 'static> + Send + 'static>(self, heartbeat_error_handler: F, connector: Connector) -> Box<Future<Item = (lapin::client::Client<AMQPStream<TlsStream>>, lapin::client::HeartbeatHandle), Error = Error> + Send + 'static> {
        match self.parse::<AMQPUri>() {
            Ok(uri)  => uri.connect_cancellable(heartbeat_error_handler, connector),
            Err(err) => Box::new(futures::future::err(ErrorKind::UriParsingError(err).into())),
        }
    }
}

impl<TlsStream: AsyncRead + AsyncWrite + Send + 'static> AMQPStream<TlsStream> {
    fn from_amqp_uri<Connector: FnOnce(String, TcpStream) -> Box<Future<Item = Box<TlsStream>, Error = io::Error> + Send + 'static> + Send + 'static>(uri: &AMQPUri, connector: Connector) -> Box<Future<Item = Self, Error = Error> + Send + 'static> {
        match uri.scheme {
            AMQPScheme::AMQP  => AMQPStream::raw(uri.authority.host.clone(), uri.authority.port),
            AMQPScheme::AMQPS => AMQPStream::tls(uri.authority.host.clone(), uri.authority.port, connector),
        }
    }

    fn raw(host: String, port: u16) -> Box<Future<Item = Self, Error = Error> + Send + 'static> {
        Box::new(open_tcp_stream(host, port).map(AMQPStream::Raw))
    }

    fn tls<Connector: FnOnce(String, TcpStream) -> Box<Future<Item = Box<TlsStream>, Error = io::Error> + Send + 'static> + Send + 'static>(host: String, port: u16, connector: Connector) -> Box<Future<Item = Self, Error = Error> + Send + 'static> {
        Box::new(
            open_tcp_stream(host.clone(), port).and_then(move |stream| {
                connector(host, stream).map(AMQPStream::Tls).map_err(|e| ErrorKind::ConnectionFailed(e).into())
            })
        )
    }
}

impl<TlsStream: AsyncRead + AsyncWrite + Send + 'static> Read for AMQPStream<TlsStream> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match *self {
            AMQPStream::Raw(ref mut raw) => raw.read(buf),
            AMQPStream::Tls(ref mut tls) => tls.read(buf),
        }
    }
}

impl<TlsStream: AsyncRead + AsyncWrite + Send + 'static> AsyncRead for AMQPStream<TlsStream> {
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

impl<TlsStream: AsyncRead + AsyncWrite + Send + 'static> Write for AMQPStream<TlsStream> {
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

impl<TlsStream: AsyncRead + AsyncWrite + Send + 'static> AsyncWrite for AMQPStream<TlsStream> {
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

fn open_tcp_stream(host: String, port: u16) -> Box<Future<Item = TcpStream, Error = Error> + Send + 'static> {
    let host2 = host.clone();
    Box::new(
        futures::future::result(AsyncResolver::from_system_conf()).and_then(move |(resolver, background)| {
            tokio_executor::spawn(background);
            resolver.lookup_ip(host.as_str())
        }).map_err(|e| ErrorKind::InvalidDomainName(e.to_string()).into()).and_then(|response| {
            response.iter().next().ok_or_else(|| ErrorKind::InvalidDomainName(host2).into())
        }).and_then(move |ipaddr| {
            TcpStream::connect(&SocketAddr::new(ipaddr, port)).map_err(|e| ErrorKind::ConnectionFailed(e).into())
        })
    )
}

fn connect_stream<T: AsyncRead + AsyncWrite + Send + Sync + 'static, F: FnOnce(Error) + Send + 'static>(stream: T, uri: AMQPUri, heartbeat_error_handler: F, create_heartbeat_handle: bool) -> Box<Future<Item = (lapin::client::Client<T>, Option<lapin::client::HeartbeatHandle>), Error = Error> + Send + 'static> {
    Box::new(lapin::client::Client::connect(stream, ConnectionOptions::from_uri(uri)).map(move |(client, mut heartbeat_future)| {
        let heartbeat_handle = if create_heartbeat_handle { heartbeat_future.handle() } else { None };
        tokio_executor::spawn(heartbeat_future.map_err(|e| heartbeat_error_handler(e.into())));
        (client, heartbeat_handle)
    }).map_err(|e| ErrorKind::ProtocolError(e).into()))
}
