#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/lapin-futures-tls-api/0.12.0/")]

//! lapin-futures-tls-api
//!
//! This library offers a nice integration of `tls-api` with the `lapin-futures` library.
//! It uses `amq-protocol` URI parsing feature and adds the `connect` and `connect_cancellable`
//! methods to `AMQPUri` which will provide you with a `lapin_futures::client::Client` and
//! optionally a `lapin_futures::client::HeartbeatHandle` wrapped in a `Future`.
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
//! extern crate tokio;
//!
//! use lapin_futures_tls_api::lapin;
//!
//! use futures::future::Future;
//! use lapin::channel::ConfirmSelectOptions;
//! use lapin_futures_tls_api::AMQPConnectionTlsApiExt;
//!
//! fn main() {
//!     env_logger::init();
//!
//!     tokio::run(
//!         "amqps://user:pass@host/vhost?heartbeat=10".connect_cancellable::<tls_api_stub::TlsConnector>(|err| {
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

extern crate futures;
extern crate lapin_futures_tls_internal;
extern crate tls_api;
extern crate tokio_tcp;
extern crate tokio_tls_api;

/// Reexport of the `lapin_futures` crate
pub mod lapin;
/// Reexport of the `uri` module from the `amq_protocol` crate
pub mod uri;

use std::io;

use futures::future::Future;
use lapin_futures_tls_internal::{AMQPConnectionTlsExt, AMQPStream};
use tls_api::{TlsConnector, TlsConnectorBuilder};
use tokio_tcp::TcpStream;
use tokio_tls_api::TlsStream;

use uri::AMQPUri;

fn connector<C: TlsConnector + Send + 'static>(host: String, stream: TcpStream) -> Box<Future<Item = Box<TlsStream<TcpStream>>, Error = io::Error> + Send + 'static> {
    Box::new(futures::future::result(C::builder().and_then(TlsConnectorBuilder::build).map_err(From::from)).and_then(move |connector| {
        tokio_tls_api::connect_async(&connector, &host, stream).map_err(From::from).map(Box::new)
    }))
}

/// Add a connect method providing a `lapin_futures::client::Client` wrapped in a `Future`.
pub trait AMQPConnectionTlsApiExt<F: FnOnce(io::Error) + Send + 'static> : AMQPConnectionTlsExt<TlsStream<TcpStream>, F> where Self: Sized {
    /// Method providing a `lapin_futures::client::Client` wrapped in a `Future`
    fn connect<C: TlsConnector + Send + 'static>(self, heartbeat_error_handler: F) -> Box<Future<Item = lapin::client::Client<AMQPStream<TlsStream<TcpStream>>>, Error = io::Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect(self, heartbeat_error_handler, connector::<C>)
    }
    /// Method providing a `lapin_futures::client::Client` and `lapin_futures::client::HeartbeatHandle` wrapped in a `Future`
    fn connect_cancellable<C: TlsConnector + Send + 'static>(self, heartbeat_error_handler: F) -> Box<Future<Item = (lapin::client::Client<AMQPStream<TlsStream<TcpStream>>>, lapin::client::HeartbeatHandle), Error = io::Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect_cancellable(self, heartbeat_error_handler, connector::<C>)
    }
}

impl<F: FnOnce(io::Error) + Send + 'static> AMQPConnectionTlsApiExt<F> for AMQPUri {}
impl<'a, F: FnOnce(io::Error) + Send + 'static> AMQPConnectionTlsApiExt<F> for &'a str {}
