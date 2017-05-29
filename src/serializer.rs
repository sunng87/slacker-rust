use serde::Serialize;

use serde_json;
use serde_json::value::Value as Json;

pub trait Serializer: Send + Sync + 'static {
    type Format: Send + Sync + 'static;

    fn serialize(&self, t: &Self::Format) -> Option<Vec<u8>>;

    fn deserialize(&self, f: &[u8]) -> Option<Self::Format>;

    fn deserialize_vec(&self, f: &[u8]) -> Option<Vec<Self::Format>>;
}

pub struct JsonSerializer;

impl Serializer for JsonSerializer {
    type Format = Json;

    fn serialize(&self, t: &Self::Format) -> Option<Vec<u8>> {
        serde_json::to_string(t)
            .ok()
            .map(|r| r.as_bytes().into())
    }

    fn deserialize(&self, f: &[u8]) -> Option<Self::Format> {
        serde_json::from_slice(f).ok()
    }

    fn deserialize_vec(&self, f: &[u8]) -> Option<Vec<Self::Format>> {
        self.deserialize(f)
            .and_then(|v| match v {
                          Json::Array(a) => Some(a),
                          _ => None,
                      })
    }
}
