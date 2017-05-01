#[macro_use]
extern crate log;

extern crate slacker;
extern crate serde_json;
extern crate futures;
extern crate env_logger;

use serde_json::value::Value as Json;
use slacker::{ThreadPoolServer, JsonRpcFnSync};

use std::sync::Arc;
use std::collections::BTreeMap;

fn echo(s: &Vec<Json>) -> Json {
    debug!("calling {:?}", s);
    Json::Array(s.clone())
}

fn main() {
    drop(env_logger::init());

    let mut funcs: BTreeMap<String, JsonRpcFnSync> = BTreeMap::new();
    funcs.insert("rust.test/echo".to_owned(), Arc::new(echo));
    let addr = "127.0.0.1:3299".parse().unwrap();
    let server = ThreadPoolServer::new(addr, funcs, 10);
    server.serve();
}
