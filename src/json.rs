use serde_json::value::Value as Json;

use service::*;

pub type JsonRpcFn = RpcFn<Json>;
pub type JsonRpcFnSync = RpcFnSync<Json>;

pub struct JsonSlacker;
