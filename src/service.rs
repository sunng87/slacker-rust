use std::io;
use std::collections::BTreeMap;
use std::sync::Arc;

use futures::Future;
use futures::future::{ok, err, BoxFuture};
use futures::sync::oneshot::Receiver;
use futures_cpupool::CpuPool;
use tservice::{NewService, Service};

use serde::Serialize;

//use packets::*;
use parser::*;
use serializer::*;

pub type RpcFn<T> = Box<Fn(&Vec<T>) -> Receiver<T> + Send + Sync + 'static>;
pub type RpcFnSync<T> = Arc<Fn(&Vec<T>) -> T + Send + Sync + 'static>;

pub struct SlackerService<T>
    where T: Serialize + Send + Sync + 'static
{
    functions: Arc<BTreeMap<String, RpcFn<T>>>,
    serializer: Arc<Serializer<Format = T>>,
}

impl<T> SlackerService<T>
    where T: Serialize + Send + Sync
{
    pub fn new(functions: Arc<BTreeMap<String, RpcFn<T>>>,
               serializer: Arc<Serializer<Format = T>>)
               -> SlackerService<T> {
        SlackerService {
            functions,
            serializer,
        }
    }
}

impl<T> Service for SlackerService<T>
    where T: Serialize + Send + Sync {
    type Request = SlackerPacket;
    type Response = SlackerPacket;
    type Error = io::Error;
    type Future = BoxFuture<Self::Response, Self::Error>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let SlackerPacket(header, body) = req;
        match body {
            SlackerPacketBody::Request(sreq) => {
                debug!("getting request: {:?}", sreq.fname);
                if let Some(f) = self.functions.get(&sreq.fname) {
                    let s = self.serializer.clone();
                    if let Some(args) = s.deserialize_vec(&sreq.arguments) {
                        f(&args)
                            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Oneshot canceled"))
                            .and_then(move |r| {
                                debug!("getting results");
                                if let Some(result) = s.serialize(&r) {
                                    let mut resp_header = header.clone();
                                    resp_header.packet_type = PACKET_TYPE_RESPONSE;
                                    debug!("sending results");
                                    ok(SlackerPacket(resp_header,
                                                      SlackerPacketBody::Response(SlackerResponsePacket {
                                                          result_code: RESULT_CODE_SUCCESS,
                                                          content_type: sreq.content_type,
                                                          data: result,
                                                      })))
                                } else {
                                    err(io::Error::new(io::ErrorKind::Other, "Unsupport"))
                                }
                            }).boxed()
                    } else {
                        err(io::Error::new(io::ErrorKind::Other, "Unsupported content type"))
                            .boxed()
                    }
                } else {
                    ok(SlackerPacket(header,
                                     SlackerPacketBody::Error(SlackerErrorPacket {
                                         result_code:
                                         RESULT_CODE_NOT_FOUND,
                                     })))
                        .boxed()
                }
            }
            SlackerPacketBody::Ping => {
                let mut resp_header = header.clone();
                resp_header.packet_type = PACKET_TYPE_PONG;
                ok(SlackerPacket(resp_header, SlackerPacketBody::Pong)).boxed()
            }
            _ => err(io::Error::new(io::ErrorKind::InvalidInput, "Unsupported packet")).boxed(),
        }
    }
}

pub struct NewSlackerService<T>(pub Arc<BTreeMap<String, RpcFn<T>>>, pub Arc<Serializer<Format = T>>)
    where T: Serialize + Send + Sync + 'static;

impl<T> NewService for NewSlackerService<T>
    where T: Serialize + Send + Sync + 'static
{
    type Request = SlackerPacket;
    type Response = SlackerPacket;
    type Error = io::Error;
    type Instance = SlackerService<T>;

    fn new_service(&self) -> io::Result<Self::Instance> {
        Ok(SlackerService::new(self.0.clone(), self.1.clone()))
    }
}

pub struct SlackerServiceSync<T>
    where T: Send + Sync + 'static
{
    functions: Arc<BTreeMap<String, RpcFnSync<T>>>,
    serializer: Arc<Serializer<Format = T>>,
    threads: usize,
    pool: CpuPool,
}

impl<T> SlackerServiceSync<T>
    where T: Send + Sync + 'static
{
    pub fn new(functions: Arc<BTreeMap<String, RpcFnSync<T>>>,
               serializer: Arc<Serializer<Format = T>>,
               threads: usize)
               -> SlackerServiceSync<T> {
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
    where T: Send + Sync + 'static
{
    type Request = SlackerPacket;
    type Response = SlackerPacket;
    type Error = io::Error;
    type Future = BoxFuture<Self::Response, Self::Error>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let SlackerPacket(header, body) = req;
        match body {
            SlackerPacketBody::Request(sreq) => {
                debug!("getting request: {:?}", sreq.fname);
                if let Some(fi) = self.functions.get(&sreq.fname) {
                    let f = fi.clone();
                    let s = self.serializer.clone();

                    self.pool
                        .spawn_fn(move || -> Result<Self::Response, Self::Error> {
                            s.deserialize_vec(&sreq.arguments)
                                .and_then(|v| Some(f(&v)))
                                .and_then(|v| s.serialize(&v))
                                .map(move |result| {
                                    debug!("getting results");
                                    let mut resp_header = header.clone();
                                    resp_header.packet_type = PACKET_TYPE_RESPONSE;
                                    SlackerPacket(header,
                                                  SlackerPacketBody::Response(SlackerResponsePacket {
                                                      result_code: RESULT_CODE_SUCCESS,
                                                      content_type: sreq.content_type,
                                                      data: result,
                                                  }))
                                })
                                .ok_or(io::Error::new(io::ErrorKind::Other, "Unsupported"))
                        }).boxed()
                } else {
                    ok(SlackerPacket(header,
                                     SlackerPacketBody::Error(SlackerErrorPacket {
                                                                  result_code:
                                                                      RESULT_CODE_NOT_FOUND,
                                                              })))
                            .boxed()
                }
            }
            SlackerPacketBody::Ping => {
                let mut resp_header = header.clone();
                resp_header.packet_type = PACKET_TYPE_PONG;
                ok(SlackerPacket(header, SlackerPacketBody::Pong)).boxed()
            }
            _ => err(io::Error::new(io::ErrorKind::InvalidInput, "Unsupported packet")).boxed(),
        }
    }
}

pub struct NewSlackerServiceSync<T>(pub Arc<BTreeMap<String, RpcFnSync<T>>>, pub Arc<Serializer<Format=T>>, pub usize)
    where T: Send + Sync + 'static;
 
impl<T> NewService for NewSlackerServiceSync<T>
    where T: Send + Sync + 'static
{
    type Request = SlackerPacket;
    type Response = SlackerPacket;
    type Error = io::Error;
    type Instance = SlackerServiceSync<T>;

    fn new_service(&self) -> io::Result<Self::Instance> {
        Ok(SlackerServiceSync::new(self.0.clone(), self.1.clone(), self.2))
    }
}
