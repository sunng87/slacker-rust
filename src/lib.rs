#![allow(dead_code, unused_must_use)]
#[macro_use]
extern crate log;
#[macro_use]
extern crate nom;

extern crate tokio_io as tio;
extern crate tokio_core as tcore;
extern crate tokio_proto as tproto;
extern crate tokio_service as tservice;
extern crate futures;
extern crate futures_cpupool;
extern crate serde;
extern crate serde_json;
extern crate bytes;
extern crate byteorder;

mod codecs;
mod parser;
mod service;
mod serializer;
mod client;
mod json;

use tproto::TcpServer;
use tproto::multiplex::{ClientProto, ServerProto};
use tio::AsyncRead;
use tio::codec::Framed;
use tcore::net::TcpStream;

use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;
use std::net::SocketAddr;

use codecs::*;
use serializer::*;
use parser::*;
use service::*;
use json::*;
pub use json::{JsonRpcFn, JsonRpcFnSync};
pub use client::{Client, ClientManager};

impl ServerProto<TcpStream> for JsonSlacker {
    type Request = SlackerPacket;
    type Response = SlackerPacket;
    type Transport = Framed<TcpStream, SlackerCodec>;
    type BindTransport = io::Result<Self::Transport>;

    fn bind_transport(&self, io: TcpStream) -> Self::BindTransport {
        io.set_nodelay(true);
        Ok(io.framed(SlackerCodec))
    }
}

pub struct Server {
    addr: SocketAddr,
    funcs: Arc<BTreeMap<String, JsonRpcFn>>,
}

impl Server {
    pub fn new(addr: SocketAddr, funcs: BTreeMap<String, JsonRpcFn>) -> Self {
        Server {
            addr,
            funcs: Arc::new(funcs),
        }
    }

    pub fn serve(&self) {
        let serializer = Arc::new(JsonSerializer);
        let new_service = NewSlackerService(self.funcs.clone(), serializer);
        TcpServer::new(JsonSlacker, self.addr).serve(new_service);
    }
}

pub struct ThreadPoolServer {
    addr: SocketAddr,
    funcs: Arc<BTreeMap<String, JsonRpcFnSync>>,
    threads: usize,
}

impl ThreadPoolServer {
    pub fn new(addr: SocketAddr, funcs: BTreeMap<String, JsonRpcFnSync>, threads: usize) -> Self {
        ThreadPoolServer {
            addr,
            funcs: Arc::new(funcs),
            threads,
        }
    }

    pub fn serve(&self) {
        let serializer = Arc::new(JsonSerializer);
        let new_service = NewSlackerServiceSync(self.funcs.clone(), serializer, self.threads);
        TcpServer::new(JsonSlacker, self.addr).serve(new_service);
    }
}

impl ClientProto<TcpStream> for JsonSlacker {
    type Request = SlackerPacket;
    type Response = SlackerPacket;
    type Transport = Framed<TcpStream, SlackerCodec>;
    type BindTransport = io::Result<Self::Transport>;

    fn bind_transport(&self, io: TcpStream) -> Self::BindTransport {
        io.set_nodelay(true);
        Ok(io.framed(SlackerCodec))
    }
}
