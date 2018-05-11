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
//! extern crate tokio_core;
//!
//! use lapin_futures_native_tls::lapin;
//!
//! use futures::future::Future;
//! use lapin::channel::ConfirmSelectOptions;
//! use lapin_futures_native_tls::AMQPConnectionNativeTlsExt;
//! use tokio_core::reactor::Core;
//!
//! fn main() {
//!     env_logger::init();
//!
//!     let mut core = Core::new().unwrap();
//!     let handle   = core.handle();
//!
//!     core.run(
//!         "amqps://user:pass@host/vhost?heartbeat=10".connect(handle, |_| ()).and_then(|client| {
//!             println!("Connected!");
//!             client.create_confirm_channel(ConfirmSelectOptions::default())
//!         }).and_then(|channel| {
//!             println!("Closing channel.");
//!             channel.close(200, "Bye")
//!         })
//!     ).unwrap();
//! }
//! ```

extern crate futures;
extern crate lapin_futures_tls_api;
extern crate tls_api_native_tls;
extern crate tokio_core;

/// Reexport of the `lapin_futures` crate
pub mod lapin;
/// Reexport of the `uri` module from the `amq_protocol` crate
pub mod uri;

use std::io;

use futures::future::Future;
use lapin_futures_tls_api::{AMQPConnectionExt, AMQPStream};
use tokio_core::reactor::Handle;

use uri::AMQPUri;

/// Add a connect method providing a `lapin_futures::client::Client` wrapped in a `Future`.
pub trait AMQPConnectionNativeTlsExt: AMQPConnectionExt {
    /// Method providing a `lapin_futures::client::Client` wrapped in a `Future`
    /// using a `tokio_code::reactor::Handle`.
    fn connect<F: FnOnce(io::Error) + 'static>(&self, handle: Handle, heartbeat_error_handler: F) -> Box<Future<Item = lapin::client::Client<AMQPStream>, Error = io::Error> + 'static> {
        AMQPConnectionExt::connect::<tls_api_native_tls::TlsConnector, _>(self, handle, heartbeat_error_handler)
    }
}

impl AMQPConnectionNativeTlsExt for AMQPUri {}
impl AMQPConnectionNativeTlsExt for str {}
