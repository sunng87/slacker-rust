use std::io;
use std::collections::BTreeMap;
use std::sync::Arc;

use futures::Future;
use futures::future::{ok, err, BoxFuture, FutureResult};
use futures::sync::oneshot::Receiver;
use futures_cpupool::CpuPool;
use tservice::{NewService, Service};

use packets::*;

pub type RpcFn<T> = Box<Fn(&Vec<T>) -> Receiver<T> + Send + Sync + 'static>;
pub type RpcFnSync<T> = Box<Fn(&Vec<T>) -> T + Send + Sync + 'static>;

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

pub struct SlackerServiceSync<T>
    where T: Send + Sync + 'static
{
    functions: Arc<BTreeMap<String, RpcFnSync<T>>>,
    pool: CpuPool,
}

impl<T> SlackerServiceSync<T>
    where T: Send + Sync + 'static
{
    pub fn new(functions: Arc<BTreeMap<String, RpcFnSync<T>>>,
               threads: usize)
               -> SlackerServiceSync<T> {
        let pool = CpuPool::new(threads);
        SlackerServiceSync { functions, pool }
    }
}

impl<T> Service for SlackerServiceSync<T>
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
                if let Some(_) = self.functions.get(&sreq.fname) {
                    self.pool
                        .spawn_fn(move || -> FutureResult<T, Self::Error> {
                                      let f = self.functions.get(&sreq.fname).unwrap();
                                      ok(f(&sreq.arguments))
                                  })
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

pub struct NewSlackerService<T>(pub Arc<BTreeMap<String, RpcFn<T>>>) where T: Send + Sync + 'static;

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
