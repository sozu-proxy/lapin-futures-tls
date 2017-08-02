extern crate env_logger;
extern crate futures;
extern crate lapin_futures_tls_api;
extern crate tls_api_stub;
extern crate tokio_core;

use lapin_futures_tls_api::lapin;

use futures::future::Future;
use lapin::channel::ConfirmSelectOptions;
use lapin_futures_tls_api::AMQPConnectionExt;
use tokio_core::reactor::Core;

fn main() {
    env_logger::init().unwrap();

    let mut core = Core::new().unwrap();
    let handle   = core.handle();

    core.run(
        "amqps://user:pass@host/vhost?heartbeat=10".connect::<tls_api_stub::TlsConnector>(handle).and_then(|client| {
            println!("Connected!");
            client.create_confirm_channel(ConfirmSelectOptions::default())
        }).and_then(|channel| {
            println!("Closing channel.");
            channel.close(200, "Bye")
        })
    ).unwrap();
}
