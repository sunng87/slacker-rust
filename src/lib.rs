#![allow(dead_code, unused_must_use)]
#[macro_use]
extern crate log;

extern crate tokio_core as tcore;
extern crate tokio_proto as tproto;
extern crate tokio_service as tservice;
extern crate futures;
extern crate rustc_serialize;
extern crate bytes;
extern crate byteorder;

mod packets;
mod codecs;

use bytes::BlockBuf;
use tservice::{NewService, Service};
use tproto::{server, Framed};
use tproto::pipeline::{Server, Message};
use tcore::reactor::Core;
use futures::{Future, finished, Oneshot, failed, empty, BoxFuture, Async};
use futures::stream::Empty;
use rustc_serialize::json::Json;

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
    type Response = Message<SlackerPacket<T>, Empty<(), Self::Error>>;
    type Error = io::Error;
    type Future = BoxFuture<Self::Response, Self::Error>;
    // type Fut = Finished<SlackerPacket<T>, ()>;

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
                        .map(Message::WithoutBody)
                        .map_err(|_| io::Error::new(io::ErrorKind::Other, "Oneshot canceled"))
                        .boxed()
                } else {
                    let error = SlackerError {
                        version: sreq.version,
                        code: RESULT_CODE_NOT_FOUND,
                        serial_id: sreq.serial_id,
                    };
                    finished(SlackerPacket::Error(error)).map(Message::WithoutBody).boxed()
                }
            }
            SlackerPacket::Ping(ref ping) => {
                finished(SlackerPacket::Pong(SlackerPong { version: ping.version }))
                    .map(Message::WithoutBody)
                    .boxed()
            }
            _ => {
                Box::new(failed(io::Error::new(io::ErrorKind::InvalidInput, "Unsupported packet")))
            }
        }
    }

    fn poll_ready(&self) -> Async<()> {
        Async::Ready(())
    }
}


pub fn serve<T>(addr: SocketAddr, new_service: T) -> io::Result<()>
    where T: NewService<Request = SlackerPacket<Json>,
                        Response = Message<SlackerPacket<Json>, Empty<(), io::Error>>,
                        Error = io::Error> + Send + 'static
{
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    try!(server::listen(&handle, addr, move |socket| {
        // Create the service
        let service = try!(new_service.new_service());

        // Create the transport
        let codec = JsonSlackerCodec;
        let transport = Framed::new(socket,
                                    codec,
                                    codec,
                                    BlockBuf::default(),
                                    BlockBuf::default());

        // Return the pipeline server task
        Server::new(service, transport)
    }));

    core.run(empty::<(), ()>());
    Ok(())
}
