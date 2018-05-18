extern crate env_logger;
extern crate futures;
extern crate lapin_futures_native_tls;
extern crate tokio;

use lapin_futures_native_tls::lapin;

use futures::future::Future;
use lapin::channel::ConfirmSelectOptions;
use lapin_futures_native_tls::AMQPConnectionNativeTlsExt;

fn main() {
    env_logger::init();

    tokio::run(
        "amqps://user:pass@host/vhost?heartbeat=10".connect(|err| {
            eprintln!("heartbeat error: {:?}", err);
        }).and_then(|client| {
            println!("Connected!");
            client.create_confirm_channel(ConfirmSelectOptions::default())
        }).and_then(|channel| {
            println!("Closing channel.");
            channel.close(200, "Bye")
        }).map_err(|err| {
            eprintln!("amqp error: {:?}", err);
        })
    );
}
