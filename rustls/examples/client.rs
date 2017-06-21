extern crate env_logger;
extern crate futures;
extern crate lapin_futures_rustls;
extern crate tokio_core;

use futures::future::Future;
use lapin_futures_rustls::AMQPConnectionRustlsExt;
use tokio_core::reactor::Core;

fn main() {
    env_logger::init().unwrap();

    let mut core = Core::new().unwrap();
    let handle   = core.handle();

    core.run(
        futures::future::ok(()).and_then(|_| {
        "amqps://user:pass@host/vhost?heartbeat=10".connect(&handle).and_then(|client| {
            println!("Connected!");
            client.create_confirm_channel()
        }).and_then(|channel| {
            println!("Closing channel.");
            channel.close(200, "Bye".to_string())
        })
        })
    ).unwrap();
}
