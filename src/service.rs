use std::io;
use std::collections::BTreeMap;
use std::sync::Arc;

use futures::{IntoFuture, Future};
use futures::future::{ok, err};
use futures::sync::oneshot::Receiver;
use futures_cpupool::CpuPool;
use tservice::Service;

use serde::Serialize;

use parser::*;
use serializer::*;

pub type RpcFn<T> = Box<Fn(&Vec<T>) -> Receiver<T> + Send + Sync + 'static>;
pub type RpcFnSync<T> = Arc<Fn(&Vec<T>) -> T + Send + Sync + 'static>;

pub struct SlackerService<T>
where
    T: Serialize + Send + Sync + 'static,
{
    functions: Arc<BTreeMap<String, RpcFn<T>>>,
    serializer: Arc<Serializer<Format = T>>,
}

impl<T> SlackerService<T>
where
    T: Serialize + Send + Sync,
{
    pub fn new(
        functions: Arc<BTreeMap<String, RpcFn<T>>>,
        serializer: Arc<Serializer<Format = T>>,
    ) -> SlackerService<T> {
        SlackerService {
            functions,
            serializer,
        }
    }
}

impl<T> Service for SlackerService<T>
where
    T: Serialize + Send + Sync,
{
    type Request = SlackerPacket;
    type Response = SlackerPacket;
    type Error = io::Error;
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let SlackerPacket(header, body) = req;
        match body {
            SlackerPacketBody::Request(sreq) => {
                debug!("getting request: {:?}", sreq.fname);
                if let Some(f) = self.functions.get(&sreq.fname) {
                    let s = self.serializer.clone();
                    match s.deserialize_vec(&sreq.arguments) {
                        Ok(args) => {
                            Box::new(f(&args)
                                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
                                .and_then(move |r| s.serialize(&r).into_future())
                                .and_then(move |result| {
                                    let mut resp_header = header.clone();
                                    resp_header.packet_type = PACKET_TYPE_RESPONSE;
                                    debug!("sending results");
                                    let body = SlackerPacketBody::Response(SlackerResponsePacket {
                                        result_code: RESULT_CODE_SUCCESS,
                                        content_type: sreq.content_type,
                                        data: result,
                                    });
                                    ok(SlackerPacket(resp_header, body))
                                }))
                        }
                        Err(e) => Box::new(err(e))
                    }
                } else {
                    Box::new(ok(SlackerPacket(
                        header,
                        SlackerPacketBody::Error(
                            SlackerErrorPacket { result_code: RESULT_CODE_NOT_FOUND },
                        ),
                    )))
                }
            }
            SlackerPacketBody::Ping => {
                let mut resp_header = header.clone();
                resp_header.packet_type = PACKET_TYPE_PONG;
                Box::new(ok(SlackerPacket(resp_header, SlackerPacketBody::Pong)))
            }
            _ => {
                Box::new(err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Unsupported packet",
                )))
            }
        }
    }
}

pub struct SlackerServiceSync<T>
where
    T: Serialize + Send + Sync + 'static,
{
    functions: Arc<BTreeMap<String, RpcFnSync<T>>>,
    serializer: Arc<Serializer<Format = T>>,
    threads: usize,
    pool: CpuPool,
}

impl<T> SlackerServiceSync<T>
where
    T: Serialize + Send + Sync + 'static,
{
    pub fn new(
        functions: Arc<BTreeMap<String, RpcFnSync<T>>>,
        serializer: Arc<Serializer<Format = T>>,
        threads: usize,
    ) -> SlackerServiceSync<T> {
        let pool = CpuPool::new(threads);
        SlackerServiceSync {
            functions,
            serializer,
            threads,
            pool,
        }
    }
}

impl<T> Service for SlackerServiceSync<T>
where
    T: Serialize + Send + Sync + 'static,
{
    type Request = SlackerPacket;
    type Response = SlackerPacket;
    type Error = io::Error;
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let SlackerPacket(header, body) = req;
        match body {
            SlackerPacketBody::Request(sreq) => {
                debug!("getting request: {:?}", sreq.fname);
                if let Some(fi) = self.functions.get(&sreq.fname) {
                    let f = fi.clone();
                    let s = self.serializer.clone();

                    Box::new(self.pool
                        .spawn_fn(move || -> Result<Self::Response, Self::Error> {
                            s.deserialize_vec(&sreq.arguments)
                                .map(|v| f(&v))
                                .and_then(|v| s.serialize(&v))
                                .map(move |result| {
                                    debug!("getting results");
                                    let mut resp_header = header.clone();
                                    resp_header.packet_type = PACKET_TYPE_RESPONSE;
                                    let body = SlackerPacketBody::Response(SlackerResponsePacket {
                                        result_code: RESULT_CODE_SUCCESS,
                                        content_type: sreq.content_type,
                                        data: result,
                                    });
                                    SlackerPacket(resp_header, body)
                                })
                        }))
                } else {
                    Box::new(ok(SlackerPacket(
                        header,
                        SlackerPacketBody::Error(
                            SlackerErrorPacket { result_code: RESULT_CODE_NOT_FOUND },
                        ),
                    )))
                }
            }
            SlackerPacketBody::Ping => {
                let mut resp_header = header.clone();
                resp_header.packet_type = PACKET_TYPE_PONG;
                Box::new(ok(SlackerPacket(header, SlackerPacketBody::Pong)))
            }
            _ => {
                Box::new(err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Unsupported packet",
                )))
            }
        }
    }
}
