#[macro_use]
extern crate log;

extern crate slacker;
extern crate serde_json;
extern crate futures;
extern crate env_logger;

use futures::{oneshot, Oneshot};
use serde_json::value::Value as Json;
use slacker::serve;

use std::collections::BTreeMap;

fn echo(s: &Vec<Json>) -> Oneshot<Json> {
    debug!("calling {:?}", s);
    let (c, p) = oneshot::<Json>();
    c.complete(Json::Array(s.clone()));
    p
}

fn main() {
    drop(env_logger::init());

    let mut funcs = BTreeMap::new();
    funcs.insert("rust.test/echo".to_owned(),
                 Box::new(echo) as Box<Fn(&Vec<Json>) -> Oneshot<Json> + Sync + Send>);
    let addr = "127.0.0.1:3299".parse().unwrap();
    serve(addr, funcs);
}
