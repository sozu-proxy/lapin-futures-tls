#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/lapin-futures-rustls/0.16.0/")]

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
//! extern crate env_logger;
//! extern crate futures;
//! extern crate lapin_futures_rustls;
//! extern crate tokio;
//!
//! use lapin_futures_rustls::lapin;
//!
//! use futures::future::Future;
//! use lapin::channel::ConfirmSelectOptions;
//! use lapin_futures_rustls::AMQPConnectionRustlsExt;
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

extern crate futures;
extern crate lapin_futures_tls_internal;
extern crate tokio_rustls;
extern crate tokio_tcp;
extern crate webpki_roots;

/// Reexport of the `lapin_futures` crate
pub mod lapin;
/// Reexport of the `uri` module from the `amq_protocol` crate
pub mod uri;

use tokio_rustls::{rustls, webpki};

use std::io;
use std::sync::Arc;

use futures::future::Future;
use lapin_futures_tls_internal::{AMQPConnectionTlsExt, AMQPStream};
use rustls::{ClientConfig, ClientSession};
use tokio_tcp::TcpStream;
use tokio_rustls::{TlsConnector, TlsStream};

use uri::AMQPUri;

fn connector(host: String, stream: TcpStream) -> Box<Future<Item = Box<TlsStream<TcpStream, ClientSession>>, Error = io::Error> + Send + 'static> {
    let mut config = ClientConfig::new();
    config.root_store.add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
    let config = TlsConnector::from(Arc::new(config));

    Box::new(futures::future::result(webpki::DNSNameRef::try_from_ascii_str(&host).map(move |domain| domain.to_owned()).map_err(|()| io::Error::new(io::ErrorKind::Other, "Invalid domain name"))).and_then(move |domain| {
        config.connect(domain.as_ref(), stream).map_err(From::from).map(Box::new)
    }))
}

/// Add a connect method providing a `lapin_futures::client::Client` wrapped in a `Future`.
pub trait AMQPConnectionRustlsExt<F: FnOnce(io::Error) + Send + 'static> : AMQPConnectionTlsExt<TlsStream<TcpStream, ClientSession>, F> where Self: Sized {
    /// Method providing a `lapin_futures::client::Client` wrapped in a `Future`
    fn connect(self, heartbeat_error_handler: F) -> Box<Future<Item = lapin::client::Client<AMQPStream<TlsStream<TcpStream, ClientSession>>>, Error = io::Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect(self, heartbeat_error_handler, connector)
    }
    /// Method providing a `lapin_futures::client::Client` and `lapin_futures::client::HeartbeatHandle` wrapped in a `Future`
    fn connect_cancellable(self, heartbeat_error_handler: F) -> Box<Future<Item = (lapin::client::Client<AMQPStream<TlsStream<TcpStream, ClientSession>>>, lapin::client::HeartbeatHandle), Error = io::Error> + Send + 'static> {
        AMQPConnectionTlsExt::connect_cancellable(self, heartbeat_error_handler, connector)
    }
}

impl<F: FnOnce(io::Error) + Send + 'static> AMQPConnectionRustlsExt<F> for AMQPUri {}
impl<'a, F: FnOnce(io::Error) + Send + 'static> AMQPConnectionRustlsExt<F> for &'a str {}
