#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/lapin-futures-native-tls/0.1.0/")]

//! lapin-futures-native-tls
//!
//! This library offers a nice integration of `native-tls` with the `lapin-futures` library.
//! It uses `amq-protocol` URI parsing feature and adds a `connect` method to `AMQPUri`
//! which will provide you with a `lapin_futures::client::Client` wrapped in a `Future`.
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
//!         "amqps://user:pass@host/vhost?heartbeat=10".connect(|err| {
//!             eprintln!("heartbeat error: {:?}", err);
//!         }).and_then(|client| {
//!             println!("Connected!");
//!             client.create_confirm_channel(ConfirmSelectOptions::default())
//!         }).and_then(|channel| {
//!             println!("Closing channel.");
//!             channel.close(200, "Bye")
//!         }).map_err(|err| {
//!             eprintln!("amqp error: {:?}", err);
//!         })
//!     );
//! }
//! ```

extern crate futures;
extern crate lapin_futures_tls_api;
extern crate tls_api_native_tls;

/// Reexport of the `lapin_futures` crate
pub mod lapin;
/// Reexport of the `uri` module from the `amq_protocol` crate
pub mod uri;

use std::io;

use futures::future::Future;
use lapin_futures_tls_api::{AMQPConnectionExt, AMQPStream};

use uri::AMQPUri;

/// Add a connect method providing a `lapin_futures::client::Client` wrapped in a `Future`.
pub trait AMQPConnectionNativeTlsExt<F: FnOnce(io::Error) + Send + 'static>: AMQPConnectionExt<F> where Self: Sized {
    /// Method providing a `lapin_futures::client::Client` wrapped in a `Future`
    fn connect(self, heartbeat_error_handler: F) -> Box<Future<Item = lapin::client::Client<AMQPStream>, Error = io::Error> + Send + 'static> {
        AMQPConnectionExt::connect::<tls_api_native_tls::TlsConnector>(self, heartbeat_error_handler)
    }
}

impl<F: FnOnce(io::Error) + Send + 'static> AMQPConnectionNativeTlsExt<F> for AMQPUri {}
impl<'a, F: FnOnce(io::Error) + Send + 'static> AMQPConnectionNativeTlsExt<F> for &'a str {}
