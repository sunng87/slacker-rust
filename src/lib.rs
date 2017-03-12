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

use tproto::{TcpClient, TcpServer};
use tproto::multiplex::{ClientProto, ServerProto, ClientService};
use tcore::io::{Io, Framed};
use tcore::net::TcpStream;
use tcore::reactor::Handle;
use futures::{Future, BoxFuture};
use tservice::Service;
use serde_json::value::Value as Json;

use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};
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

impl ClientProto<TcpStream> for JsonSlacker {
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

pub struct Client {
    inner: ClientService<TcpStream, JsonSlacker>,
    serial_id_gen: AtomicIsize,
}

impl Service for Client {
    type Request = SlackerPacket<Json>;
    type Response = SlackerPacket<Json>;
    type Error = io::Error;
    type Future = BoxFuture<Self::Response, Self::Error>;

    fn call(&self, req: Self::Request) -> Self::Future {
        self.inner.call(req).boxed()
    }
}

impl Client {
    pub fn connect(addr: &SocketAddr,
                   handle: &Handle)
                   -> Box<Future<Item = Client, Error = io::Error>> {
        let rt = TcpClient::new(JsonSlacker)
                     .connect(addr, handle)
                     .map(|client_service| {
                         Client {
                             inner: client_service,
                             serial_id_gen: AtomicIsize::new(0),
                         }
                     });
        Box::new(rt)
    }

    pub fn rpc_call(&self,
                    ns_name: &str,
                    fn_name: &str,
                    args: Vec<Json>)
                    -> BoxFuture<Json, io::Error> {
        let mut fname = String::new();
        fname.push_str(ns_name);
        fname.push_str("/");
        fname.push_str(fn_name);

        let sid = self.serial_id_gen.fetch_add(1, Ordering::SeqCst) as i32;

        let packet = SlackerPacket::Request(SlackerRequest {
            version: PROTOCOL_VERSION,
            serial_id: sid,
            content_type: SlackerContentType::JSON,
            fname: fname,
            arguments: args,
        });
        self.call(packet)
            .map(|t| {
                match t {
                    SlackerPacket::Response(r) => r.result,
                    _ => Json::Null,
                }
            })
            .boxed()
    }
}
