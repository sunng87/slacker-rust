#![allow(dead_code, unused_must_use)]
#[macro_use]
extern crate log;

extern crate tokio_core as tcore;
extern crate tokio_proto as tproto;
extern crate tokio_service as tservice;
extern crate futures;
extern crate serde_json;
extern crate byteorder;

mod packets;
mod codecs;
mod service;

use tproto::TcpServer;
use tproto::multiplex::ServerProto;
use tcore::io::{Io, Framed};
use tcore::net::TcpStream;
use serde_json::value::Value as Json;

use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;
use std::net::SocketAddr;

use packets::*;
use codecs::*;
use service::*;

struct JsonSlacker;

impl ServerProto<TcpStream> for JsonSlacker {
    type Request = SlackerPacket<Json>;
    type Response = SlackerPacket<Json>;
    type Transport = Framed<TcpStream, JsonSlackerCodec>;
    type BindTransport = io::Result<Self::Transport>;

    fn bind_transport(&self, io: TcpStream) -> Self::BindTransport {
        io.set_nodelay(true);
        Ok(io.framed(JsonSlackerCodec))
    }
}

pub fn serve(addr: SocketAddr, funcs: BTreeMap<String, RpcFn<Json>>) {
    let new_service = NewSlackerService(Arc::new(funcs));
    TcpServer::new(JsonSlacker, addr).serve(new_service);
}
