use env_logger;
use futures::{self, future::Future};
use lapin_futures_tls_internal::{AMQPConnectionTlsExt, lapin};
use lapin::channel::ConfirmSelectOptions;
use native_tls;
use tokio;
use tokio_tls::TlsConnector;

use std::io;

fn main() {
    env_logger::init();

    tokio::run(
        "amqps://user:pass@host/vhost?heartbeat=10".connect_cancellable(|err| {
            eprintln!("heartbeat error: {:?}", err);
        }, |host, stream| {
            Box::new(futures::future::result(native_tls::TlsConnector::builder().build().map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to create connector"))).and_then(move |connector| {
                TlsConnector::from(connector).connect(&host, stream).map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to connect")).map(Box::new)
            }))
        }).and_then(|(client, heartbeat_handle)| {
            println!("Connected!");
            client.create_confirm_channel(ConfirmSelectOptions::default()).map(|channel| (channel, heartbeat_handle)).and_then(|(channel, heartbeat_handle)| {
                println!("Stopping heartbeat.");
                heartbeat_handle.stop();
                println!("Closing channel.");
                channel.close(200, "Bye")
            }).map_err(From::from)
        }).map_err(|err| {
            eprintln!("amqp error: {:?}", err);
        })
    );
}
