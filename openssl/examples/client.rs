extern crate env_logger;
extern crate futures;
extern crate lapin_futures_openssl;
extern crate tokio_core;

use lapin_futures_openssl::lapin;

use futures::future::Future;
use lapin::channel::ConfirmSelectOptions;
use lapin_futures_openssl::AMQPConnectionOpensslExt;
use tokio_core::reactor::Core;

fn main() {
    env_logger::init();

    let mut core = Core::new().unwrap();
    let handle   = core.handle();

    core.run(
        "amqps://user:pass@host/vhost?heartbeat=10".connect(handle, |_| ()).and_then(|client| {
            println!("Connected!");
            client.create_confirm_channel(ConfirmSelectOptions::default())
        }).and_then(|channel| {
            println!("Closing channel.");
            channel.close(200, "Bye")
        })
    ).unwrap();
}
