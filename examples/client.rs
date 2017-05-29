extern crate amq_protocol;
extern crate env_logger;
extern crate futures;
extern crate lapin_futures_rustls;
extern crate tokio_core;

use amq_protocol::uri::AMQPUri;
use futures::future::Future;
use lapin_futures_rustls::AMQPConnectionExt;
use tokio_core::reactor::Core;

fn main() {
    env_logger::init().unwrap();

    let uri      = "amqps://user:pass@host/vhost?heartbeat=10".parse::<AMQPUri>().unwrap();
    let mut core = Core::new().unwrap();
    let handle   = core.handle();

    core.run(
        uri.connect(&handle).and_then(|client| {
            println!("Connected!");
            client.create_confirm_channel()
        }).and_then(|channel| {
            println!("Closing channel.");
            channel.close(200, "Bye".to_string())
        })
    ).unwrap();
}
