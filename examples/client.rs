extern crate futures;
extern crate slacker;
#[macro_use]
extern crate serde_json;
extern crate env_logger;
extern crate tokio_core as tcore;

use futures::Future;
use tcore::reactor::Core;

use slacker::ClientManager;

fn main() {
    drop(env_logger::init());

    let mut core = Core::new().unwrap();

    let manager = ClientManager::new();

    let addr = "127.0.0.1:3299".parse().unwrap();
    let client = manager.connect(&mut core, &addr);

    core.run(
        client
            .and_then(|c| c.rpc_call("rust.test", "echo", vec![json!(1), json!(2)]))
            .and_then(|r| {
                println!("{:?}", r);
                Ok(())
            }),
    ).unwrap();
}
