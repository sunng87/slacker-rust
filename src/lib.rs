#![allow(dead_code, unused_must_use)]
#[macro_use]
extern crate log;
#[macro_use]
extern crate nom;

extern crate byteorder;
extern crate bytes;
extern crate futures;
extern crate futures_cpupool;
extern crate serde;
extern crate serde_json;
extern crate tokio;
extern crate tokio_codec;

mod client;
mod codecs;
mod json;
mod parser;
mod serializer;
mod service;

use tokio::net::{TcpStream, TcpListener};

use std::collections::BTreeMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

pub use client::{Client, ClientManager};
use codecs::*;
use json::*;
pub use json::{JsonRpcFn, JsonRpcFnSync};
use parser::*;
use serializer::*;
use service::*;

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
        let funcs_ref = self.funcs.clone();
        TcpServer::new(JsonSlacker, self.addr)
            .serve(move || Ok(SlackerService::new(funcs_ref.clone(), serializer.clone())));
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
        let funcs_ref = self.funcs.clone();
        let threads = self.threads;
        TcpServer::new(JsonSlacker, self.addr).serve(move || {
            Ok(SlackerServiceSync::new(
                funcs_ref.clone(),
                serializer.clone(),
                threads,
            ))
        });
    }
}
