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

use tservice::Service;
use tproto::pipeline::ServerProto;
use tcore::io::{Io, Framed};
use tcore::reactor::Core;
use futures::{future, Future, finished, Oneshot, failed, BoxFuture, Async};
use serde_json::value::Value as Json;

use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;
use std::net::SocketAddr;

use packets::*;
use codecs::*;

#[derive(Clone)]
pub struct SlackerService<T>
    where T: Send + Sync + 'static
{
    functions: Arc<BTreeMap<String, Box<Fn(&Vec<T>) -> Oneshot<T> + Send + Sync + 'static>>>,
}

impl<T> SlackerService<T>
    where T: Send + Sync + 'static
{
    pub fn new(functions: BTreeMap<String, Box<Fn(&Vec<T>) -> Oneshot<T> + Send + Sync + 'static>>)
               -> SlackerService<T> {
        SlackerService { functions: Arc::new(functions) }
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
                            finished(SlackerPacket::Response(SlackerResponse {
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
                    finished(SlackerPacket::Error(error)).boxed()
                }
            }
            SlackerPacket::Ping(ref ping) => {
                finished(SlackerPacket::Pong(SlackerPong { version: ping.version })).boxed()
            }
            _ => failed(io::Error::new(io::ErrorKind::InvalidInput, "Unsupported packet")).boxed(),
        }
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



// pub fn serve<T>(addr: SocketAddr, new_service: T) -> io::Result<()>
//     where T: NewService<Request = SlackerPacket<Json>,
//                         Response = SlackerPacket<Json>,
//                         Error = io::Error> + Send + 'static
// {
//     let mut core = Core::new().unwrap();
//     let handle = core.handle();

//     try!(server::listen(&handle, addr, move |socket| {
//         // Create the service
//         let service = try!(new_service.new_service());

//         // Create the transport
//         let codec = JsonSlackerCodec;
//         let transport = EasyFramed::new(socket, codec, codec);

//         // Return the pipeline server task
//         Server::new(service, transport)
//     }));

//     core.run(empty::<(), ()>());
//     Ok(())
// }
