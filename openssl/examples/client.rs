extern crate env_logger;
extern crate futures;
extern crate lapin_futures_openssl;
extern crate tokio;

use lapin_futures_openssl::lapin;

use futures::future::Future;
use lapin::channel::ConfirmSelectOptions;
use lapin_futures_openssl::AMQPConnectionOpensslExt;

fn main() {
    env_logger::init();

    tokio::run(
        "amqps://user:pass@host/vhost?heartbeat=10".connect_cancellable(|err| {
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
