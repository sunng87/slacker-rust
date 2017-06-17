use std::io::{Result, Error, ErrorKind};

use serde::Serialize;

use serde_json;
use serde_json::value::Value as Json;

pub trait Serializer: Send + Sync + 'static {
    type Format: Serialize + Send + Sync + 'static;

    fn serialize(&self, t: &Self::Format) -> Result<Vec<u8>>;

    fn deserialize(&self, f: &[u8]) -> Result<Self::Format>;

    fn deserialize_vec(&self, f: &[u8]) -> Result<Vec<Self::Format>>;
}

pub struct JsonSerializer;

impl Serializer for JsonSerializer {
    type Format = Json;

    fn serialize(&self, t: &Self::Format) -> Result<Vec<u8>> {
        serde_json::to_string(t)
            .map(|r| r.as_bytes().into())
            .map_err(|e| Error::new(ErrorKind::InvalidData, e))
    }

    fn deserialize(&self, f: &[u8]) -> Result<Self::Format> {
        serde_json::from_slice(f).map_err(|e| Error::new(ErrorKind::InvalidData, e))
    }

    fn deserialize_vec(&self, f: &[u8]) -> Result<Vec<Self::Format>> {
        self.deserialize(f).and_then(|v| match v {
            Json::Array(a) => Ok(a),
            _ => Err(Error::new(ErrorKind::InvalidData, "Array required.")),
        })
    }
}
