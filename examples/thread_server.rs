#[macro_use]
extern crate log;
#[macro_use]
extern crate maplit;

extern crate env_logger;
extern crate futures;
extern crate serde_json;
extern crate slacker;

use serde_json::value::Value as Json;
use slacker::{JsonRpcFnSync, ThreadPoolServer};

use std::sync::Arc;

fn echo(s: &Vec<Json>) -> Json {
    debug!("calling {:?}", s);
    Json::Array(s.clone())
}

fn main() {
    drop(env_logger::init());

    let funcs = btreemap! {
        "rust.test/echo".to_owned() => Arc::new(echo) as JsonRpcFnSync
    };

    let addr = "127.0.0.1:3299".parse().unwrap();
    let server = ThreadPoolServer::new(addr, funcs, 10);
    server.serve();
}
