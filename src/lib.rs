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

//mod packets;
mod codecs;
mod parser;
mod service;
mod serializer;

use tproto::{TcpClient, TcpServer};
use tproto::multiplex::{ClientProto, ServerProto, ClientService};
use tio::AsyncRead;
use tio::codec::Framed;
use tcore::net::TcpStream;
use tcore::reactor::Handle;
use futures::{Future, BoxFuture};
use futures::future::err;
use tservice::Service;
use serde_json::value::Value as Json;

use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::net::SocketAddr;

//use packets::*;
use codecs::*;
use serializer::*;
use parser::*;
use service::*;

pub type JsonRpcFn = RpcFn<Json>;
pub type JsonRpcFnSync = RpcFnSync<Json>;

struct JsonSlacker;

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

pub struct Client {
    inner: ClientService<TcpStream, JsonSlacker>,
    serial_id_gen: AtomicIsize,
    // TODO: this can be reused
    serializer: Arc<JsonSerializer>,
}

impl Service for Client {
    type Request = SlackerPacket;
    type Response = SlackerPacket;
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
                         serializer: Arc::new(JsonSerializer),
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
        let header = SlackerPacketHeader {
            version: PROTOCOL_VERSION_5,
            serial_id: sid,
            packet_type: PACKET_TYPE_REQUEST,
        };

        let serializer = self.serializer.clone();
        if let Some(serialized_args) = serializer.serialize(&args.into()) {
            let packet = SlackerPacketBody::Request(SlackerRequestPacket {
                                                        content_type: JSON_CONTENT_TYPE,
                                                        fname: fname,
                                                        arguments: serialized_args,
                                                    });
            self.call(SlackerPacket(header, packet))
                .map(move |SlackerPacket(_, body)| match body {
                         SlackerPacketBody::Response(r) => {
                             serializer.deserialize(&r.data).unwrap_or(Json::Null)
                         }
                         _ => Json::Null,
                     })
                .boxed()

        } else {
            return err(io::Error::new(io::ErrorKind::Other, "Unsupported")).boxed();
        }
    }
}
