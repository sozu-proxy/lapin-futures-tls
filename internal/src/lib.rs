#![deny(missing_docs)]
#![warn(rust_2018_idioms)]
#![doc(html_root_url = "https://docs.rs/lapin-futures-tls-internal/0.6.0/")]
#![recursion_limit="128"]

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
//! use env_logger;
//! use failure::Error;
//! use futures::{self, future::Future};
//! use lapin_futures_tls_internal::{AMQPConnectionTlsExt, lapin};
//! use lapin::channel::ConfirmSelectOptions;
//! use native_tls;
//! use tokio;
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
//!         }).map_err(Error::from).and_then(|(client, heartbeat_handle)| {
//!             println!("Connected!");
//!             client.create_confirm_channel(ConfirmSelectOptions::default()).map(|channel| (channel, heartbeat_handle)).and_then(|(channel, heartbeat_handle)| {
//!                 println!("Stopping heartbeat.");
//!                 heartbeat_handle.stop();
//!                 println!("Closing channel.");
//!                 channel.close(200, "Bye")
//!             }).map_err(Error::from)
//!         }).map_err(|err| {
//!             eprintln!("amqp error: {:?}", err);
//!         })
//!     );
//! }
//! ```

/// The type errors that can be returned in this crate.
pub mod error;
/// Reexport of the `lapin_futures` crate
pub mod lapin;
/// Reexport of the `uri` module from the `amq_protocol` crate
pub mod uri;

/// Reexport of `TcpStream`
pub use tokio_tcp::TcpStream;

use bytes::{Buf, BufMut};
use failure;
use futures::{self, future::Future, Poll};
use tokio_executor;
use tokio_io::{AsyncRead, AsyncWrite};
use trust_dns_resolver::AsyncResolver;

use std::io::{self, Read, Write};
use std::net::SocketAddr;

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
pub trait AMQPConnectionTlsExt<TlsStream: AsyncRead + AsyncWrite + Send + Sync + 'static> {
    /// Method providing a `lapin_futures::client::Client`, a `lapin_futures::client::HeartbeatHandle` and a `lapin::client::Heartbeat` pulse wrapped in a `Future`
    fn connect<Connector: FnOnce(String, TcpStream) -> Box<dyn Future<Item = Box<TlsStream>, Error = io::Error> + Send + 'static> + Send + 'static>(self, connector: Connector) -> Box<dyn Future<Item = (lapin::client::Client<AMQPStream<TlsStream>>, lapin::client::HeartbeatHandle, Box<dyn Future<Item = (), Error = Error> + Send + 'static>), Error = Error> + Send + 'static>;
    /// Method providing a `lapin_futures::client::Client` and `lapin_futures::client::HeartbeatHandle` wrapped in a `Future`
    fn connect_cancellable<Connector: FnOnce(String, TcpStream) -> Box<dyn Future<Item = Box<TlsStream>, Error = io::Error> + Send + 'static> + Send + 'static, F: FnOnce(Error) + Send + 'static>(self, heartbeat_error_handler: F, connector: Connector) -> Box<dyn Future<Item = (lapin::client::Client<AMQPStream<TlsStream>>, lapin::client::HeartbeatHandle), Error = Error> + Send + 'static>;
}

impl<TlsStream: AsyncRead + AsyncWrite + Send + Sync + 'static> AMQPConnectionTlsExt<TlsStream> for AMQPUri {
    fn connect<Connector: FnOnce(String, TcpStream) -> Box<dyn Future<Item = Box<TlsStream>, Error = io::Error> + Send + 'static> + Send + 'static>(self, connector: Connector) -> Box<dyn Future<Item = (lapin::client::Client<AMQPStream<TlsStream>>, lapin::client::HeartbeatHandle, Box<dyn Future<Item = (), Error = Error> + Send + 'static>), Error = Error> + Send + 'static> {
        Box::new(AMQPStream::from_amqp_uri(&self, connector).and_then(move |stream| lapin::client::Client::connect(stream, ConnectionOptions::from_uri(self)).map(|(client, mut heartbeat)| (client, heartbeat.handle().unwrap(), Box::new(heartbeat.map_err(|e| ErrorKind::ProtocolError(e).into())) as Box<dyn Future<Item = (), Error = Error> + Send + 'static>)).map_err(|e| ErrorKind::ProtocolError(e).into())))
    }

    fn connect_cancellable<Connector: FnOnce(String, TcpStream) -> Box<dyn Future<Item = Box<TlsStream>, Error = io::Error> + Send + 'static> + Send + 'static, F: FnOnce(Error) + Send + 'static>(self, heartbeat_error_handler: F, connector: Connector) -> Box<dyn Future<Item = (lapin::client::Client<AMQPStream<TlsStream>>, lapin::client::HeartbeatHandle), Error = Error> + Send + 'static> {
        Box::new(self.connect(connector).map(move |(client, heartbeat_handle, heartbeat_future)| {
            tokio_executor::spawn(heartbeat_future.map_err(|e| heartbeat_error_handler(e)));
            (client, heartbeat_handle)
        }))
    }
}

macro_rules! try_uri (
    ($self: expr) => ({
        match $self.parse::<AMQPUri>() {
            Ok(uri) => uri,
            Err(err) => return Box::new(futures::future::err(ErrorKind::UriParsingError(err).into())),
        }
    });
);

impl<'a, TlsStream: AsyncRead + AsyncWrite + Send + Sync + 'static> AMQPConnectionTlsExt<TlsStream> for &'a str {
    fn connect<Connector: FnOnce(String, TcpStream) -> Box<dyn Future<Item = Box<TlsStream>, Error = io::Error> + Send + 'static> + Send + 'static>(self, connector: Connector) -> Box<dyn Future<Item = (lapin::client::Client<AMQPStream<TlsStream>>, lapin::client::HeartbeatHandle, Box<dyn Future<Item = (), Error = Error> + Send + 'static>), Error = Error> + Send + 'static> {
        try_uri!(self).connect(connector)
    }

    fn connect_cancellable<Connector: FnOnce(String, TcpStream) -> Box<dyn Future<Item = Box<TlsStream>, Error = io::Error> + Send + 'static> + Send + 'static, F: FnOnce(Error) + Send + 'static>(self, heartbeat_error_handler: F, connector: Connector) -> Box<dyn Future<Item = (lapin::client::Client<AMQPStream<TlsStream>>, lapin::client::HeartbeatHandle), Error = Error> + Send + 'static> {
        try_uri!(self).connect_cancellable(heartbeat_error_handler, connector)
    }
}

impl<TlsStream: AsyncRead + AsyncWrite + Send + 'static> AMQPStream<TlsStream> {
    fn from_amqp_uri<Connector: FnOnce(String, TcpStream) -> Box<dyn Future<Item = Box<TlsStream>, Error = io::Error> + Send + 'static> + Send + 'static>(uri: &AMQPUri, connector: Connector) -> Box<dyn Future<Item = Self, Error = Error> + Send + 'static> {
        match uri.scheme {
            AMQPScheme::AMQP  => AMQPStream::raw(uri.authority.host.clone(), uri.authority.port),
            AMQPScheme::AMQPS => AMQPStream::tls(uri.authority.host.clone(), uri.authority.port, connector),
        }
    }

    fn raw(host: String, port: u16) -> Box<dyn Future<Item = Self, Error = Error> + Send + 'static> {
        Box::new(open_tcp_stream(host, port).map(AMQPStream::Raw))
    }

    fn tls<Connector: FnOnce(String, TcpStream) -> Box<dyn Future<Item = Box<TlsStream>, Error = io::Error> + Send + 'static> + Send + 'static>(host: String, port: u16, connector: Connector) -> Box<dyn Future<Item = Self, Error = Error> + Send + 'static> {
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

fn open_tcp_stream(host: String, port: u16) -> Box<dyn Future<Item = TcpStream, Error = Error> + Send + 'static> {
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
