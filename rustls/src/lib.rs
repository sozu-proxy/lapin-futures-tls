#![deny(missing_docs)]
#![warn(rust_2018_idioms)]
#![doc(html_root_url = "https://docs.rs/lapin-futures-rustls/0.21.1/")]

//! lapin-futures-rustls
//!
//! This library offers a nice integration of `rustls` with the `lapin-futures` library.
//! It uses `amq-protocol` URI parsing feature and adds the `connect` and `connect_cancellable`
//! methods to `AMQPUri` which will provide you with a `lapin_futures::client::Client` and
//! optionally a `lapin_futures::client::HeartbeatHandle` wrapped in a `Future`.
//!
//! It autodetects whether you're using `amqp` or `amqps` and opens either a raw `TcpStream`
//! or a `TlsStream` using `rustls` as the SSL engine.
//!
//! ## Connecting and opening a channel
//!
//! ```rust,no_run
//! use env_logger;
//! use failure::Error;
//! use futures::future::Future;
//! use lapin_futures_rustls::{AMQPConnectionRustlsExt, lapin};
//! use lapin::channel::ConfirmSelectOptions;
//! use tokio;
//!
//! fn main() {
//!     env_logger::init();
//!
//!     tokio::run(
//!         "amqps://user:pass@host/vhost?heartbeat=10".connect_cancellable(|err| {
//!             eprintln!("heartbeat error: {:?}", err);
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

/// Reexport of the `lapin_futures_tls_internal` errors
#[deprecated(note = "use lapin directly instead")]
pub mod error;
/// Reexport of the `lapin_futures` crate
#[deprecated(note = "use lapin directly instead")]
pub mod lapin;
/// Reexport of the `uri` module from the `amq_protocol` crate
#[deprecated(note = "use lapin directly instead")]
pub mod uri;

/// Reexport of `AMQPStream`
#[deprecated(note = "use lapin directly instead")]
pub type AMQPStream = lapin_futures_tls_internal::AMQPStream<TlsStream<TcpStream, ClientSession>>;

use futures::{self, future::Future};
use lapin_futures_tls_internal::{self, AMQPConnectionTlsExt, error::Error, lapin::client::ConnectionProperties, TcpStream};
use tokio_rustls::{rustls, TlsConnector, TlsStream, webpki};
use rustls::{ClientConfig, ClientSession};
use webpki_roots;

use std::io;
use std::sync::Arc;

use uri::AMQPUri;

fn connector(host: String, stream: TcpStream) -> Box<dyn Future<Item = Box<TlsStream<TcpStream, ClientSession>>, Error = io::Error> + Send + 'static> {
    let mut config = ClientConfig::new();
    config.root_store.add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
    let config = TlsConnector::from(Arc::new(config));

    Box::new(futures::future::result(webpki::DNSNameRef::try_from_ascii_str(&host).map(move |domain| domain.to_owned()).map_err(|()| io::Error::new(io::ErrorKind::Other, "Invalid domain name"))).and_then(move |domain| {
        config.connect(domain.as_ref(), stream).map_err(From::from).map(Box::new)
    }))
}

/// Add a connect method providing a `lapin_futures::client::Client` wrapped in a `Future`.
#[deprecated(note = "use lapin directly instead")]
pub trait AMQPConnectionRustlsExt: AMQPConnectionTlsExt<TlsStream<TcpStream, ClientSession>> where Self: Sized {
    /// Method providing a `lapin_futures::client::Client`, a `lapin_futures::client::HeartbeatHandle` and a `lapin::client::Heartbeat` pulse wrapped in a `Future`
    fn connect(self) -> Box<dyn Future<Item = (lapin::client::Client<AMQPStream>, lapin::client::HeartbeatHandle, Box<dyn Future<Item = (), Error = Error> + Send + 'static>), Error = Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect(self, connector)
    }
    /// Method providing a `lapin_futures::client::Client` and `lapin_futures::client::HeartbeatHandle` wrapped in a `Future`
    fn connect_cancellable<F: FnOnce(Error) + Send + 'static>(self, heartbeat_error_handler: F) -> Box<dyn Future<Item = (lapin::client::Client<AMQPStream>, lapin::client::HeartbeatHandle), Error = Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect_cancellable(self, heartbeat_error_handler, connector)
    }
    /// Method providing a `lapin_futures::client::Client`, a `lapin_futures::client::HeartbeatHandle` and a `lapin::client::Heartbeat` pulse wrapped in a `Future`
    fn connect_full(self, properties: ConnectionProperties) -> Box<dyn Future<Item = (lapin::client::Client<AMQPStream>, lapin::client::HeartbeatHandle, Box<dyn Future<Item = (), Error = Error> + Send + 'static>), Error = Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect_full(self, connector, properties)
    }
    /// Method providing a `lapin_futures::client::Client` and `lapin_futures::client::HeartbeatHandle` wrapped in a `Future`
    fn connect_cancellable_full<F: FnOnce(Error) + Send + 'static>(self, heartbeat_error_handler: F, properties: ConnectionProperties) -> Box<dyn Future<Item = (lapin::client::Client<AMQPStream>, lapin::client::HeartbeatHandle), Error = Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect_cancellable_full(self, heartbeat_error_handler, connector, properties)
    }
}

impl AMQPConnectionRustlsExt for AMQPUri {}
impl<'a> AMQPConnectionRustlsExt for &'a str {}
