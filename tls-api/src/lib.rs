#![deny(missing_docs)]
#![warn(rust_2018_idioms)]
#![doc(html_root_url = "https://docs.rs/lapin-futures-tls-api/0.19.1/")]

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
//! use env_logger;
//! use failure::Error;
//! use futures::future::Future;
//! use lapin_futures_tls_api::{AMQPConnectionTlsApiExt, lapin};
//! use lapin::channel::ConfirmSelectOptions;
//! use tokio;
//!
//! fn main() {
//!     env_logger::init();
//!
//!     tokio::run(
//!         "amqps://user:pass@host/vhost?heartbeat=10".connect_cancellable::<tls_api_stub::TlsConnector, _>(|err| {
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
pub type AMQPStream = lapin_futures_tls_internal::AMQPStream<TlsStream<TcpStream>>;

use futures::{self, future::Future};
use lapin_futures_tls_internal::{self, AMQPConnectionTlsExt, error::Error, lapin::client::ConnectionProperties, TcpStream};
use tls_api::{TlsConnector, TlsConnectorBuilder};
use tokio_tls_api::{self, TlsStream};

use std::io;

use uri::AMQPUri;

fn connector<C: TlsConnector + Send + 'static>(host: String, stream: TcpStream) -> Box<dyn Future<Item = Box<TlsStream<TcpStream>>, Error = io::Error> + Send + 'static> {
    Box::new(futures::future::result(C::builder().and_then(TlsConnectorBuilder::build).map_err(From::from)).and_then(move |connector| {
        tokio_tls_api::connect_async(&connector, &host, stream).map_err(From::from).map(Box::new)
    }))
}

/// Add a connect method providing a `lapin_futures::client::Client` wrapped in a `Future`.
#[deprecated(note = "use lapin directly instead")]
pub trait AMQPConnectionTlsApiExt: AMQPConnectionTlsExt<TlsStream<TcpStream>> where Self: Sized {
    /// Method providing a `lapin_futures::client::Client`, a `lapin_futures::client::HeartbeatHandle` and a `lapin::client::Heartbeat` pulse wrapped in a `Future`
    fn connect<C: TlsConnector + Send + 'static>(self) -> Box<dyn Future<Item = (lapin::client::Client<AMQPStream>, lapin::client::HeartbeatHandle, Box<dyn Future<Item = (), Error = Error> + Send + 'static>), Error = Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect(self, connector::<C>)
    }
    /// Method providing a `lapin_futures::client::Client` and `lapin_futures::client::HeartbeatHandle` wrapped in a `Future`
    fn connect_cancellable<C: TlsConnector + Send + 'static, F: FnOnce(Error) + Send + 'static>(self, heartbeat_error_handler: F) -> Box<dyn Future<Item = (lapin::client::Client<AMQPStream>, lapin::client::HeartbeatHandle), Error = Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect_cancellable(self, heartbeat_error_handler, connector::<C>)
    }
    /// Method providing a `lapin_futures::client::Client`, a `lapin_futures::client::HeartbeatHandle` and a `lapin::client::Heartbeat` pulse wrapped in a `Future`
    fn connect_full<C: TlsConnector + Send + 'static>(self, properties: ConnectionProperties) -> Box<dyn Future<Item = (lapin::client::Client<AMQPStream>, lapin::client::HeartbeatHandle, Box<dyn Future<Item = (), Error = Error> + Send + 'static>), Error = Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect_full(self, connector::<C>, properties)
    }
    /// Method providing a `lapin_futures::client::Client` and `lapin_futures::client::HeartbeatHandle` wrapped in a `Future`
    fn connect_cancellable_full<C: TlsConnector + Send + 'static, F: FnOnce(Error) + Send + 'static>(self, heartbeat_error_handler: F, properties: ConnectionProperties) -> Box<dyn Future<Item = (lapin::client::Client<AMQPStream>, lapin::client::HeartbeatHandle), Error = Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect_cancellable_full(self, heartbeat_error_handler, connector::<C>, properties)
    }
}

impl AMQPConnectionTlsApiExt for AMQPUri {}
impl<'a> AMQPConnectionTlsApiExt for &'a str {}
