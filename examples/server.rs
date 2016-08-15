#[macro_use]
extern crate log;

extern crate slacker;
extern crate tokio;
extern crate rustc_serialize;
extern crate futures;
extern crate env_logger;

use futures::{promise, Promise};
use rustc_serialize::json::Json;
use slacker::{serve, SlackerService};

use std::collections::BTreeMap;
use std::net::SocketAddr;

fn echo(s: &Vec<Json>) -> Promise<Json> {
    debug!("calling {:?}", s);
    let (c, p) = promise::<Json>();
    c.complete(Json::Array(s.clone()));
    p
}

fn main() {
    env_logger::init().unwrap();

    let mut funcs = BTreeMap::new();
    funcs.insert("rust.test/echo".to_owned(),
                 Box::new(echo) as Box<Fn(&Vec<Json>) -> Promise<Json> + Sync + Send>);

    let s = SlackerService::<Json>::new(funcs);

    let addr = "127.0.0.1:3299".parse().unwrap();
    serve(addr, s);
}