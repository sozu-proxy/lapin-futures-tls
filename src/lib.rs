extern crate amq_protocol;
extern crate futures;
extern crate lapin_futures as lapin;
extern crate rustls;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_rustls;
extern crate webpki_roots;

use std::io::{self, Read, Write};
use std::sync::Arc;

use amq_protocol::uri::{AMQPScheme, AMQPUri, AMQPUserInfo};
use futures::future::Future;
use futures::Poll;
use lapin::client::ConnectionOptions;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_rustls::{ClientConfigExt, TlsStream};

pub enum AMQPStream {
    Raw(TcpStream),
    Tls(TlsStream<TcpStream, rustls::ClientSession>),
}

pub trait AMQPConnectionExt {
    fn connect(&self, handle: &Handle, heartbeat: Option<u16>) -> Box<Future<Item = lapin::client::Client<AMQPStream>, Error = io::Error> + 'static>;
}

impl AMQPConnectionExt for AMQPUri {
    fn connect(&self, handle: &Handle, heartbeat: Option<u16>) -> Box<Future<Item = lapin::client::Client<AMQPStream>, Error = io::Error> + 'static> {
        let userinfo = self.authority.userinfo.clone();
        let vhost    = self.vhost.clone();
        let stream   = match self.scheme {
            AMQPScheme::AMQP  => AMQPStream::raw(handle, &self.authority.host, self.authority.port),
            AMQPScheme::AMQPS => AMQPStream::tls(handle, &self.authority.host, self.authority.port),
        };

        Box::new(stream.and_then(move |stream| connect_stream(stream, userinfo, vhost, heartbeat)))
    }
}

impl AMQPStream {
    fn raw(handle: &Handle, host: &str, port: u16) -> Box<Future<Item = Self, Error = io::Error> + 'static> {
        match open_tcp_stream(handle, host, port) {
            Ok(stream) => Box::new(futures::future::ok(AMQPStream::Raw(stream))),
            Err(e)     => Box::new(futures::future::err(e)),
        }
    }

    fn tls(handle: &Handle, host: &str, port: u16) -> Box<Future<Item = Self, Error = io::Error> + 'static> {
        let mut config = rustls::ClientConfig::new();
        config.root_store.add_trust_anchors(&webpki_roots::ROOTS);
        let config     = Arc::new(config);

        match open_tcp_stream(handle, host, port) {
            Ok(stream) => Box::new(config.connect_async(host, stream).map(AMQPStream::Tls)),
            Err(e)     => Box::new(futures::future::err(e)),
        }
    }
}

impl Read for AMQPStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match *self {
            AMQPStream::Raw(ref mut raw) => raw.read(buf),
            AMQPStream::Tls(ref mut tls) => tls.read(buf),
        }
    }
}

impl AsyncRead for AMQPStream {
}

impl Write for AMQPStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match *self {
            AMQPStream::Raw(ref mut raw) => raw.write(buf),
            AMQPStream::Tls(ref mut tls) => tls.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match *self {
            AMQPStream::Raw(ref mut raw) => raw.flush(),
            AMQPStream::Tls(ref mut tls) => tls.flush(),
        }
    }
}

impl AsyncWrite for AMQPStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        match *self {
            AMQPStream::Raw(ref mut raw) => raw.shutdown(),
            AMQPStream::Tls(ref mut tls) => tls.shutdown(),
        }
    }
}

fn open_tcp_stream(handle: &Handle, host: &str, port: u16) -> io::Result<TcpStream> {
    std::net::TcpStream::connect((host, port)).and_then(|stream| TcpStream::from_stream(stream, &handle))
}

fn connect_stream<T: AsyncRead + AsyncWrite + 'static>(stream: T, credentials: AMQPUserInfo, vhost: String, heartbeat: Option<u16>) -> Box<Future<Item = lapin::client::Client<T>, Error = io::Error> + 'static> {
    Box::new(lapin::client::Client::connect(stream, &ConnectionOptions {
        username:  credentials.username,
        password:  credentials.password,
        vhost:     vhost,
        heartbeat: heartbeat.unwrap_or_else(|| ConnectionOptions::default().heartbeat),
    }))
}
