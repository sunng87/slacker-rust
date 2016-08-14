extern crate slacker;
extern crate tokio;
extern crate rustc_serialize;
extern crate futures;

use futures::{promise, Promise};
use rustc_serialize::json::Json;
use slacker::{serve, SlackerService};

use std::collections::BTreeMap;
use std::net::SocketAddr;

fn echo(s: &Vec<Json>) -> Promise<Json> {
    let (c, p) = promise::<Json>();
    c.complete(Json::Array(s.clone()));
    p
}

fn main() {
    let mut funcs = BTreeMap::new();
    funcs.insert("echo".to_owned(),
                 Box::new(echo) as Box<Fn(&Vec<Json>) -> Promise<Json> + Sync + Send>);

    let s = SlackerService::<Json>::new(funcs);

    let addr = "0.0.0.0:8080".parse().unwrap();
    serve(addr, s);
}
