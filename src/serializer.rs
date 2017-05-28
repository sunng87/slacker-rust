use serde::Serialize;

use serde_json;
use serde_json::value::Value as Json;

pub trait Serializer {
    type Format;

    fn serialize(&self, t: &Self::Format) -> Option<Vec<u8>>;

    fn deserialize(&self, f: &[u8]) -> Option<Self::Format>;
}

pub struct JsonSerializer;

impl Serializer for JsonSerializer {
    type Format = Json;

    fn serialize(&self, t: &Self::Format) -> Option<Vec<u8>> {
        serde_json::to_string(t).ok().map(|r| r.as_bytes())
    }

    fn deserialize(&self, f: &[u8]) -> Option<Self::Format> {
        serde_json::from_slice(f).ok()
    }
}
