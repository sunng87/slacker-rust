#[macro_use]
extern crate log;
#[macro_use]
extern crate maplit;

extern crate slacker;
extern crate serde_json;
extern crate futures;
extern crate env_logger;

use futures::{oneshot, Oneshot};
use serde_json::value::Value as Json;
use slacker::{Server, JsonRpcFn};

fn echo(s: &Vec<Json>) -> Oneshot<Json> {
    debug!("calling {:?}", s);
    let (c, p) = oneshot::<Json>();
    c.send(Json::Array(s.clone())).unwrap();
    p
}

fn main() {
    drop(env_logger::init());

    let funcs =
        btreemap! {
        "rust.test/echo".to_owned() => Box::new(echo) as JsonRpcFn
    };

    let addr = "127.0.0.1:3299".parse().unwrap();
    let server = Server::new(addr, funcs);
    server.serve();
}
