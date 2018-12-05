#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/lapin-futures-native-tls/0.6.0/")]

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

extern crate futures;
extern crate lapin_futures_tls_internal;
extern crate native_tls;
extern crate tokio_tls;

/// Reexport of the `lapin_futures_tls_internal` errors
pub mod error;
/// Reexport of the `lapin_futures` crate
pub mod lapin;
/// Reexport of the `uri` module from the `amq_protocol` crate
pub mod uri;

/// Reexport of `AMQPStream` 
pub type AMQPStream = lapin_futures_tls_internal::AMQPStream<TlsStream<TcpStream>>;

use std::io;

use futures::future::Future;
use lapin_futures_tls_internal::{AMQPConnectionTlsExt, TcpStream};
use lapin_futures_tls_internal::error::Error;
use tokio_tls::{TlsConnector, TlsStream};

use uri::AMQPUri;

fn connector(host: String, stream: TcpStream) -> Box<Future<Item = Box<TlsStream<TcpStream>>, Error = io::Error> + Send + 'static> {
    Box::new(futures::future::result(native_tls::TlsConnector::builder().build().map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to create connector"))).and_then(move |connector| {
        TlsConnector::from(connector).connect(&host, stream).map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to connect")).map(Box::new)
    }))
}

/// Add a connect method providing a `lapin_futures::client::Client` wrapped in a `Future`.
pub trait AMQPConnectionNativeTlsExt<F: FnOnce(Error) + Send + 'static> : AMQPConnectionTlsExt<TlsStream<TcpStream>, F> where Self: Sized {
    /// Method providing a `lapin_futures::client::Client` wrapped in a `Future`
    fn connect(self, heartbeat_error_handler: F) -> Box<Future<Item = lapin::client::Client<AMQPStream>, Error = Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect(self, heartbeat_error_handler, connector)
    }
    /// Method providing a `lapin_futures::client::Client` and `lapin_futures::client::HeartbeatHandle` wrapped in a `Future`
    fn connect_cancellable(self, heartbeat_error_handler: F) -> Box<Future<Item = (lapin::client::Client<AMQPStream>, lapin::client::HeartbeatHandle), Error = Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect_cancellable(self, heartbeat_error_handler, connector)
    }
}

impl<F: FnOnce(Error) + Send + 'static> AMQPConnectionNativeTlsExt<F> for AMQPUri {}
impl<'a, F: FnOnce(Error) + Send + 'static> AMQPConnectionNativeTlsExt<F> for &'a str {}
