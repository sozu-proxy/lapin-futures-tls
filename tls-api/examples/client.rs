extern crate env_logger;
extern crate futures;
extern crate lapin_futures_tls_api;
extern crate tls_api_stub;
extern crate tokio;

use lapin_futures_tls_api::lapin;

use futures::future::Future;
use lapin::channel::ConfirmSelectOptions;
use lapin_futures_tls_api::AMQPConnectionTlsApiExt;

fn main() {
    env_logger::init();

    tokio::run(
        "amqps://user:pass@host/vhost?heartbeat=10".connect_cancellable::<tls_api_stub::TlsConnector>(|err| {
            eprintln!("heartbeat error: {:?}", err);
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
