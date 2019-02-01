use env_logger;
use failure::Error;
use futures::future::Future;
use lapin_futures_native_tls::{AMQPConnectionNativeTlsExt, lapin};
use lapin::channel::ConfirmSelectOptions;
use tokio;

fn main() {
    env_logger::init();

    tokio::run(
        "amqps://user:pass@host/vhost?heartbeat=10".connect_cancellable(|err| {
            eprintln!("heartbeat error: {:?}", err);
        }).map_err(Error::from).and_then(|(client, heartbeat_handle)| {
            println!("Connected!");
            client.create_confirm_channel(ConfirmSelectOptions::default()).map(|channel| (channel, heartbeat_handle)).and_then(|(channel, heartbeat_handle)| {
                println!("Stopping heartbeat.");
                heartbeat_handle.stop();
                println!("Closing channel.");
                channel.close(200, "Bye")
            }).map_err(Error::from)
        }).map_err(|err| {
            eprintln!("amqp error: {:?}", err);
        })
    );
}
