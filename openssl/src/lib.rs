#![deny(missing_docs)]
#![warn(rust_2018_idioms)]
#![doc(html_root_url = "https://docs.rs/lapin-futures-openssl/0.9.0/")]

//! lapin-futures-openssl
//!
//! This library offers a nice integration of `openssl` with the `lapin-futures` library.
//! It uses `amq-protocol` URI parsing feature and adds the `connect` and `connect_cancellable`
//! methods to `AMQPUri` which will provide you with a `lapin_futures::client::Client` and
//! optionally a `lapin_futures::client::HeartbeatHandle` wrapped in a `Future`.
//!
//! It autodetects whether you're using `amqp` or `amqps` and opens either a raw `TcpStream`
//! or a `SslStream` using `openssl` as the SSL engine.
//!
//! ## Connecting and opening a channel
//!
//! ```rust,no_run
//! use env_logger;
//! use failure::Error;
//! use futures::future::Future;
//! use lapin_futures_openssl::{AMQPConnectionOpensslExt, lapin};
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
pub mod error;
/// Reexport of the `lapin_futures` crate
pub mod lapin;
/// Reexport of the `uri` module from the `amq_protocol` crate
pub mod uri;

/// Reexport of `AMQPStream`
pub type AMQPStream = lapin_futures_tls_internal::AMQPStream<SslStream<TcpStream>>;

use futures::{self, future::Future};
use lapin_futures_tls_internal::{self, AMQPConnectionTlsExt, error::Error, TcpStream};
use openssl::ssl::{SslConnector, SslMethod};
use tokio_openssl::{SslConnectorExt, SslStream};

use std::io;

use uri::AMQPUri;

fn connector(host: String, stream: TcpStream) -> Box<dyn Future<Item = Box<SslStream<TcpStream>>, Error = io::Error> + Send + 'static> {
    Box::new(futures::future::result(SslConnector::builder(SslMethod::tls()).map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to create connector"))).and_then(move |connector| {
        connector.build().connect_async(&host, stream).map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to connect")).map(Box::new)
    }))
}

/// Add a connect method providing a `lapin_futures::client::Client` wrapped in a `Future`.
pub trait AMQPConnectionOpensslExt: AMQPConnectionTlsExt<SslStream<TcpStream>> where Self: Sized {
    /// Method providing a `lapin_futures::client::Client` wrapped in a `Future`
    fn connect(self) -> Box<dyn Future<Item = lapin::client::Client<AMQPStream>, Error = Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect(self, connector)
    }
    /// Method providing a `lapin_futures::client::Client` and `lapin_futures::client::HeartbeatHandle` wrapped in a `Future`
    fn connect_cancellable<F: FnOnce(Error) + Send + 'static>(self, heartbeat_error_handler: F) -> Box<dyn Future<Item = (lapin::client::Client<AMQPStream>, lapin::client::HeartbeatHandle), Error = Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect_cancellable(self, heartbeat_error_handler, connector)
    }
    /// Method providing a `lapin_futures::client::Client`, a `lapin_futures::client::HeartbeatHandle` and a `lapin::client::Heartbeat` pulse wrapped in a `Future`
    fn connect_raw(self) -> Box<dyn Future<Item = (lapin::client::Client<AMQPStream>, lapin::client::HeartbeatHandle, Box<dyn Future<Item = (), Error = Error> + Send + 'static>), Error = Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect_raw(self, connector)
    }
}

impl AMQPConnectionOpensslExt for AMQPUri {}
impl<'a> AMQPConnectionOpensslExt for &'a str {}
