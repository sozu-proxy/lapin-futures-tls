extern crate amq_protocol;
extern crate futures;
extern crate lapin_futures as lapin;
extern crate rustls;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_rustls;
extern crate webpki_roots;

use std::io;
use std::sync::Arc;

use amq_protocol::uri::{AMQPScheme, AMQPUri, AMQPUserInfo};
use futures::future::Future;
use lapin::client::ConnectionOptions;
use rustls::ClientConfig;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Core;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_rustls::ClientConfigExt;

pub trait AMQPConnectionExt {
    fn connect_raw(&self, core: &Core, heartbeat: Option<u16>) -> Box<Future<Item = lapin::client::Client<TcpStream>, Error = io::Error> + 'static>;
    fn connect_tls(&self, core: &Core, heartbeat: Option<u16>) -> Box<Future<Item = lapin::client::Client<tokio_rustls::TlsStream<tokio_core::net::TcpStream, rustls::ClientSession>>, Error = io::Error> + 'static>;
}

impl AMQPConnectionExt for AMQPUri {
    fn connect_raw(&self, core: &Core, heartbeat: Option<u16>) -> Box<Future<Item = lapin::client::Client<TcpStream>, Error = io::Error> + 'static> {
        if self.scheme != AMQPScheme::AMQP {
            return Box::new(futures::future::err(io::Error::new(io::ErrorKind::Other, "connect_raw called but scheme is not 'amqp'")))
        }
        let userinfo = self.authority.userinfo.clone();
        let vhost    = self.vhost.clone();
        Box::new(raw_stream(core, self.authority.host.as_str(), self.authority.port).and_then(move |stream| connect_stream(stream, userinfo, vhost, heartbeat)))
    }

    fn connect_tls(&self, core: &Core, heartbeat: Option<u16>) -> Box<Future<Item = lapin::client::Client<tokio_rustls::TlsStream<tokio_core::net::TcpStream, rustls::ClientSession>>, Error = io::Error> + 'static> {
        if self.scheme != AMQPScheme::AMQPS {
            return Box::new(futures::future::err(io::Error::new(io::ErrorKind::Other, "connect_tls called but scheme is not 'amqps'")))
        }
        let mut config = ClientConfig::new();
        config.root_store.add_trust_anchors(&webpki_roots::ROOTS);
        let config     = Arc::new(config);
        let host       = self.authority.host.clone();
        let userinfo   = self.authority.userinfo.clone();
        let vhost      = self.vhost.clone();
        Box::new(raw_stream(core, self.authority.host.as_str(), self.authority.port).and_then(move |stream| config.connect_async(&host, stream)).and_then(move |stream| connect_stream(stream, userinfo, vhost, heartbeat)))
    }
}

fn raw_stream(core: &Core, host: &str, port: u16) -> Box<Future<Item = TcpStream, Error = io::Error> + 'static> {
    let handle     = core.handle();

    match std::net::TcpStream::connect((host, port)).and_then(|stream| TcpStream::from_stream(stream, &handle)) {
        Ok(stream) => Box::new(futures::future::ok(stream)),
        Err(e)     => Box::new(futures::future::err(e)),
    }
}

fn connect_stream<T: AsyncRead + AsyncWrite + 'static>(stream: T, credentials: AMQPUserInfo, vhost: String, heartbeat: Option<u16>) -> Box<Future<Item = lapin::client::Client<T>, Error = io::Error> + 'static> {
    Box::new(lapin::client::Client::connect(stream, &ConnectionOptions {
        username:  credentials.username,
        password:  credentials.password,
        vhost:     vhost,
        heartbeat: heartbeat.unwrap_or_else(|| ConnectionOptions::default().heartbeat),
    }))
}
