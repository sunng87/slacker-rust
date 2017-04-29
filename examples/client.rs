extern crate slacker;
extern crate futures;
extern crate tokio_core;
#[macro_use]
extern crate serde_json;
extern crate env_logger;

use futures::Future;
use tokio_core::reactor::Core;
use slacker::Client;

fn main() {
    drop(env_logger::init());

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let addr = "127.0.0.1:3299".parse().unwrap();
    core.run(Client::connect(&addr, &handle)
                 .and_then(|c| c.rpc_call("rust.test", "echo", vec![json!(1), json!(2)]))
                 .and_then(|r| {
                               println!("{:?}", r);
                               Ok(())
                           }))
        .unwrap();
}
