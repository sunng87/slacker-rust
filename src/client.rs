use std::io;
use std::sync::Arc;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicIsize, Ordering};

use tproto::TcpClient;
use tproto::multiplex::ClientService;
use tcore::net::TcpStream;
use tcore::reactor::Core;
use futures::{Future, IntoFuture, BoxFuture};
use futures::future::err;
use tservice::Service;

use serde_json::value::Value as Json;

use serializer::*;
use parser::*;
use json::*;

pub struct ClientManager {
    serializer: Arc<JsonSerializer>,
}

impl ClientManager {
    pub fn new() -> ClientManager {
        let serializer = Arc::new(JsonSerializer);
        ClientManager { serializer }
    }

    pub fn connect(
        &self,
        core: &mut Core,
        addr: &SocketAddr,
    ) -> Box<Future<Item = Client, Error = io::Error>> {
        let handle = core.handle();
        let serializer = self.serializer.clone();
        let rt = TcpClient::new(JsonSlacker).connect(addr, &handle).map(
            |client_service| {
                Client {
                    inner: client_service,
                    serial_id_gen: AtomicIsize::new(0),
                    serializer,
                }
            },
        );
        Box::new(rt)
    }
}

pub struct Client {
    inner: ClientService<TcpStream, JsonSlacker>,
    serial_id_gen: AtomicIsize,
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
    pub fn rpc_call(
        &self,
        ns_name: &str,
        fn_name: &str,
        args: Vec<Json>,
    ) -> BoxFuture<Json, io::Error> {
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
        let body_result = serializer.serialize(&args.into()).map(|serialized_args| {
            SlackerPacketBody::Request(SlackerRequestPacket {
                content_type: JSON_CONTENT_TYPE,
                fname: fname,
                arguments: serialized_args,
            })
        });
        match body_result {
            Ok(body) => {
                self.call(SlackerPacket(header, body))
                    .and_then(move |SlackerPacket(_, body)| {
                        debug!("getting results {:?}", body);
                        match body {
                            SlackerPacketBody::Response(r) => {
                                serializer.deserialize(&r.data).into_future()
                            }
                            _ => {
                                err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "Unexpect packet.",
                                ))
                            }
                        }
                    })
                    .boxed()
            }
            Err(e) => err(e).boxed(),
        }
    }

    pub fn ping(&self) -> BoxFuture<(), io::Error> {
        let sid = self.serial_id_gen.fetch_add(1, Ordering::SeqCst) as i32;
        let header = SlackerPacketHeader {
            version: PROTOCOL_VERSION_5,
            serial_id: sid,
            packet_type: PACKET_TYPE_PING,
        };

        let body = SlackerPacketBody::Ping;
        self.call(SlackerPacket(header, body)).map(|_| ()).boxed()
    }
}
