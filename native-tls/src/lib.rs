#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/lapin-futures-native-tls/0.5.0/")]

//! lapin-futures-native-tls
//!
//! This library offers a nice integration of `native-tls` with the `lapin-futures` library.
//! It uses `amq-protocol` URI parsing feature and adds the `connect` and `connect_cancellable`
//! methods to `AMQPUri` which will provide you with a `lapin_futures::client::Client` and
//! optionally a `lapin_futures::client::HeartbeatHandle` wrapped in a `Future`.
//!
//! It autodetects whether you're using `amqp` or `amqps` and opens either a raw `TcpStream`
//! or a `TlsStream` using `native-tls` as the SSL engine.
//!
//! ## Connecting and opening a channel
//!
//! ```rust,no_run
//! extern crate env_logger;
//! extern crate futures;
//! extern crate lapin_futures_native_tls;
//! extern crate tokio;
//!
//! use lapin_futures_native_tls::lapin;
//!
//! use futures::future::Future;
//! use lapin::channel::ConfirmSelectOptions;
//! use lapin_futures_native_tls::AMQPConnectionNativeTlsExt;
//!
//! fn main() {
//!     env_logger::init();
//!
//!     tokio::run(
//!         "amqps://user:pass@host/vhost?heartbeat=10".connect_cancellable(|err| {
//!             eprintln!("heartbeat error: {:?}", err);
//!         }).and_then(|(client, heartbeat_handle)| {
//!             println!("Connected!");
//!             client.create_confirm_channel(ConfirmSelectOptions::default()).map(|channel| (channel, heartbeat_handle))
//!         }).and_then(|(channel, heartbeat_handle)| {
//!             println!("Stopping heartbeat.");
//!             heartbeat_handle.stop();
//!             println!("Closing channel.");
//!             channel.close(200, "Bye")
//!         }).map_err(|err| {
//!             eprintln!("amqp error: {:?}", err);
//!         })
//!     );
//! }
//! ```

extern crate bytes;
extern crate futures;
extern crate lapin_futures;
extern crate native_tls;
extern crate tokio_executor;
extern crate tokio_io;
extern crate tokio_tcp;
extern crate tokio_tls;
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
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_tls::{TlsConnector, TlsStream};
use tokio_tcp::TcpStream;
use trust_dns_resolver::ResolverFuture;

use lapin::client::ConnectionOptions;
use uri::{AMQPScheme, AMQPUri};

/// Represents either a raw `TcpStream` or a `TlsStream` backend by `tokio-tls`.
/// The `TlsStream` is wrapped in a `Box` to keep the enum footprint minimal.
pub enum AMQPStream {
    /// The raw `TcpStream` used for basic AMQP connections.
    Raw(TcpStream),
    /// The `TlsStream` used for AMQPs connections.
    Tls(Box<TlsStream<TcpStream>>),
}

/// Add a connect method providing a `lapin_futures::client::Client` wrapped in a `Future`.
pub trait AMQPConnectionNativeTlsExt<F: FnOnce(io::Error) + Send + 'static> {
    /// Method providing a `lapin_futures::client::Client` wrapped in a `Future`
    fn connect(self, heartbeat_error_handler: F) -> Box<Future<Item = lapin::client::Client<AMQPStream>, Error = io::Error> + Send + 'static>;
    /// Method providing a `lapin_futures::client::Client` and `lapin_futures::client::HeartbeatHandle` wrapped in a `Future`
    fn connect_cancellable(self, heartbeat_error_handler: F) -> Box<Future<Item = (lapin::client::Client<AMQPStream>, lapin::client::HeartbeatHandle), Error = io::Error> + Send + 'static>;
}

impl<F: FnOnce(io::Error) + Send + 'static> AMQPConnectionNativeTlsExt<F> for AMQPUri {
    fn connect(self, heartbeat_error_handler: F) -> Box<Future<Item = lapin::client::Client<AMQPStream>, Error = io::Error> + Send + 'static> {
        Box::new(AMQPStream::from_amqp_uri(&self).and_then(move |stream| connect_stream(stream, self, heartbeat_error_handler, false)).map(|(client, _)| client))
    }

    fn connect_cancellable(self, heartbeat_error_handler: F) -> Box<Future<Item = (lapin::client::Client<AMQPStream>, lapin::client::HeartbeatHandle), Error = io::Error> + Send + 'static> {
        Box::new(AMQPStream::from_amqp_uri(&self).and_then(move |stream| connect_stream(stream, self, heartbeat_error_handler, true)).map(|(client, heartbeat_handle)| (client, heartbeat_handle.unwrap())))
    }
}

impl<'a, F: FnOnce(io::Error) + Send + 'static> AMQPConnectionNativeTlsExt<F> for &'a str {
    fn connect(self, heartbeat_error_handler: F) -> Box<Future<Item = lapin::client::Client<AMQPStream>, Error = io::Error> + Send + 'static> {
        match self.parse::<AMQPUri>() {
            Ok(uri)  => uri.connect(heartbeat_error_handler),
            Err(err) => Box::new(futures::future::err(io::Error::new(io::ErrorKind::Other, err))),
        }
    }

    fn connect_cancellable(self, heartbeat_error_handler: F) -> Box<Future<Item = (lapin::client::Client<AMQPStream>, lapin::client::HeartbeatHandle), Error = io::Error> + Send + 'static> {
        match self.parse::<AMQPUri>() {
            Ok(uri)  => uri.connect_cancellable(heartbeat_error_handler),
            Err(err) => Box::new(futures::future::err(io::Error::new(io::ErrorKind::Other, err))),
        }
    }
}

impl AMQPStream {
    fn from_amqp_uri(uri: &AMQPUri) -> Box<Future<Item = Self, Error = io::Error> + Send + 'static> {
        match uri.scheme {
            AMQPScheme::AMQP  => AMQPStream::raw(uri.authority.host.clone(), uri.authority.port),
            AMQPScheme::AMQPS => AMQPStream::tls(uri.authority.host.clone(), uri.authority.port),
        }
    }

    fn raw(host: String, port: u16) -> Box<Future<Item = Self, Error = io::Error> + Send + 'static> {
        Box::new(open_tcp_stream(host, port).map(AMQPStream::Raw))
    }

    fn tls(host: String, port: u16) -> Box<Future<Item = Self, Error = io::Error> + Send + 'static> {
        Box::new(
            open_tcp_stream(host.clone(), port).join(
                futures::future::result(native_tls::TlsConnector::builder().build().map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to create connector")))
            ).and_then(move |(stream, connector)| {
                TlsConnector::from(connector).connect(&host, stream).map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to connect")).map(Box::new).map(AMQPStream::Tls)
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

fn open_tcp_stream(host: String, port: u16) -> Box<Future<Item = TcpStream, Error = io::Error> + Send + 'static> {
    let host = host.clone();

    Box::new(
        futures::future::result(ResolverFuture::from_system_conf()).flatten().and_then(move |resolver| {
            resolver.lookup_ip(host.as_str())
        }).map_err(From::from).and_then(|response| {
            response.iter().next().ok_or_else(|| io::Error::new(io::ErrorKind::AddrNotAvailable, "Couldn't resolve hostname"))
        }).and_then(move |ipaddr| {
            TcpStream::connect(&SocketAddr::new(ipaddr, port))
        })
    )
}

fn connect_stream<T: AsyncRead + AsyncWrite + Send + Sync + 'static, F: FnOnce(io::Error) + Send + 'static>(stream: T, uri: AMQPUri, heartbeat_error_handler: F, create_heartbeat_handle: bool) -> Box<Future<Item = (lapin::client::Client<T>, Option<lapin::client::HeartbeatHandle>), Error = io::Error> + Send + 'static> {
    Box::new(lapin::client::Client::connect(stream, ConnectionOptions::from_uri(uri)).map(move |(client, mut heartbeat_future)| {
        let heartbeat_handle = if create_heartbeat_handle { heartbeat_future.handle() } else { None };
        tokio_executor::spawn(heartbeat_future.map_err(heartbeat_error_handler));
        (client, heartbeat_handle)
    }))
}
