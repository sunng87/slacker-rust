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

use tservice::{NewService, Service};
use tproto::TcpServer;
use tproto::pipeline::ServerProto;
use tcore::io::{Io, Framed};
use futures::Future;
use futures::future::{ok, err, BoxFuture};
use futures::sync::oneshot::Receiver;
use serde_json::value::Value as Json;

use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;
use std::net::SocketAddr;

use packets::*;
use codecs::*;

pub type RpcFn<T> = Box<Fn(&Vec<T>) -> Receiver<T> + Send + Sync + 'static>;

pub struct SlackerService<T>
    where T: Send + Sync + 'static
{
    functions: Arc<BTreeMap<String, RpcFn<T>>>,
}

impl<T> SlackerService<T>
    where T: Send + Sync + 'static
{
    pub fn new(functions: Arc<BTreeMap<String, RpcFn<T>>>) -> SlackerService<T> {
        SlackerService { functions: functions }
    }
}

impl<T> Service for SlackerService<T>
    where T: Send + Sync + 'static
{
    type Request = SlackerPacket<T>;
    type Response = SlackerPacket<T>;
    type Error = io::Error;
    type Future = BoxFuture<Self::Response, Self::Error>;

    fn call(&self, req: Self::Request) -> Self::Future {
        match req {
            SlackerPacket::Request(sreq) => {
                debug!("getting request: {:?}", sreq.fname);
                if let Some(f) = self.functions.get(&sreq.fname) {
                    f(&sreq.arguments)
                        .and_then(move |result| {
                            debug!("getting results");
                            ok(SlackerPacket::Response(SlackerResponse {
                                version: sreq.version,
                                code: RESULT_CODE_SUCCESS,
                                content_type: sreq.content_type,
                                serial_id: sreq.serial_id,
                                result: result,
                            }))
                        })
                        .map_err(|_| io::Error::new(io::ErrorKind::Other, "Oneshot canceled"))
                        .boxed()
                } else {
                    let error = SlackerError {
                        version: sreq.version,
                        code: RESULT_CODE_NOT_FOUND,
                        serial_id: sreq.serial_id,
                    };
                    ok(SlackerPacket::Error(error)).boxed()
                }
            }
            SlackerPacket::Ping(ref ping) => {
                ok(SlackerPacket::Pong(SlackerPong { version: ping.version })).boxed()
            }
            _ => err(io::Error::new(io::ErrorKind::InvalidInput, "Unsupported packet")).boxed(),
        }
    }
}

struct NewSlackerService<T>(Arc<BTreeMap<String, RpcFn<T>>>) where T: Send + Sync + 'static;

impl<T> NewService for NewSlackerService<T>
    where T: Send + Sync + 'static
{
    type Request = SlackerPacket<T>;
    type Response = SlackerPacket<T>;
    type Error = io::Error;
    type Instance = SlackerService<T>;

    fn new_service(&self) -> io::Result<Self::Instance> {
        Ok(SlackerService::new(self.0.clone()))
    }
}

pub struct JsonSlacker;

impl<T: Io + 'static> ServerProto<T> for JsonSlacker {
    type Request = SlackerPacket<Json>;
    type Response = SlackerPacket<Json>;
    type Transport = Framed<T, JsonSlackerCodec>;
    type BindTransport = io::Result<Framed<T, JsonSlackerCodec>>;

    fn bind_transport(&self, io: T) -> io::Result<Framed<T, JsonSlackerCodec>> {
        Ok(io.framed(JsonSlackerCodec))
    }
}

pub fn serve(addr: SocketAddr, funcs: BTreeMap<String, RpcFn<Json>>) {
    let new_service = NewSlackerService(Arc::new(funcs));
    TcpServer::new(JsonSlacker, addr).serve(new_service);
}
