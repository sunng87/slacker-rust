#[macro_use]
extern crate log;
#[macro_use]
extern crate maplit;

extern crate env_logger;
extern crate futures;
extern crate serde_json;
extern crate slacker;

use futures::{oneshot, Oneshot};
use serde_json::value::Value as Json;
use slacker::{JsonRpcFn, Server};

fn echo(s: &Vec<Json>) -> Oneshot<Json> {
    debug!("calling {:?}", s);
    let (c, p) = oneshot::<Json>();
    c.send(Json::Array(s.clone())).unwrap();
    p
}

fn main() {
    drop(env_logger::init());

    let funcs = btreemap! {
        "rust.test/echo".to_owned() => Box::new(echo) as JsonRpcFn
    };

    let addr = "127.0.0.1:3299".parse().unwrap();
    let server = Server::new(addr, funcs);
    server.serve();
}
