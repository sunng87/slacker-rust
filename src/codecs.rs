stdin:275:3: 275:3 error: this file contains an un-closed delimiter
stdin:275 }
           ^
stdin:200:33: 200:34 help: did you mean to close this delimiter?
stdin:200 impl Codec for JsonSlackerCodec {
                                          ^
stdin:211:1: 211:3 error: unexpected token: `<<`
stdin:211 <<<<<<< HEAD
          ^~
use tcore::io::{EasyBuf, Codec};
use tproto::multiplex::RequestId;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use rustc_serialize::json::{self, Json};
use serde_json;
use serde_json::value::Value as Json;

use std::io::{self, Read, Write};

use packets::*;

#[derive(Copy, Clone)]
pub struct JsonSlackerCodec;


fn write_string(cur: &mut Vec<u8>, v: &str, prefix_len: usize) -> io::Result<()> {
    if prefix_len == 2 {
        try!(cur.write_u16::<BigEndian>(v.len() as u16));
    } else {
        try!(cur.write_u32::<BigEndian>(v.len() as u32));
    }

    cur.write_all(v.as_bytes())
}

macro_rules! get {
    ($expr: expr) => {
        if let Some(v) = $expr {
            v
        } else {
            return None;
        }
    }
}

struct EasyBufCursor<'a> {
    pub buf: &'a [u8],
    pub index: usize,
}

impl<'a> EasyBufCursor<'a> {
    fn new(input: &'a EasyBuf) -> EasyBufCursor<'a> {
        EasyBufCursor {
            buf: input.as_slice(),
            index: 0,
        }
    }

    fn len(&self) -> usize {
        self.buf.len()
    }

    fn remaining(&self) -> usize {
        self.len() - self.index
    }

    fn read_u8(&mut self) -> Option<u8> {
        self.buf
            .read_u8()
            .map(|t| {
                self.index = self.index + 1;
                t
            })
            .ok()
    }

    fn read_u16(&mut self) -> Option<u16> {
        self.buf
            .read_u16::<BigEndian>()
            .map(|t| {
                self.index = self.index + 2;
                t
            })
            .ok()
    }

    fn read_u32(&mut self) -> Option<u32> {
        self.buf
            .read_u32::<BigEndian>()
            .map(|t| {
                self.index = self.index + 4;
                t
            })
            .ok()
    }

    fn read_i32(&mut self) -> Option<i32> {
        self.buf
            .read_i32::<BigEndian>()
            .map(|t| {
                self.index = self.index + 4;
                t
            })
            .ok()
    }

    fn read_slice(&mut self, bytes: &mut [u8]) {
        self.buf.read(bytes).map(|s| self.index = self.index + s);
    }

    fn read_string(&mut self, prefix_len: usize) -> Option<String> {
        if self.remaining() >= prefix_len {
            let len: usize = if prefix_len == 2 {
                let raw = get!(self.read_u16());
                debug!("raw {:?}", raw);
                raw as usize
            } else {
                get!(self.read_u32()) as usize
            };
            debug!("len: {}, remaining: {}", len, self.remaining());
            if self.remaining() >= len {
                let mut b = vec!(0u8; len);
                self.read_slice(&mut b);
                String::from_utf8(b).ok()
            } else {
                None
            }
        } else {
            None
        }

    }
}

fn try_read_packet(buf: &mut EasyBuf) -> Option<SlackerPacket<Json>> {
    if buf.len() < 6 {
        return None;
    }

    let (packet, index) = {
        let mut cursor = EasyBufCursor::new(buf);

        let version = get!(cursor.read_u8());
        debug!("version {:?}", version);

        let serial_id = get!(cursor.read_i32());
        debug!("serial id {}", serial_id);

        let packet_code = get!(cursor.read_u8());
        debug!("packet code {}", packet_code);

        let p = match packet_code {
            0 => {
                if cursor.remaining() >= 3 {
                    // content type
                    let ct = get!(cursor.read_u8());
                    debug!("content-type {}", ct);

                    let fname = get!(cursor.read_string(2));

                    let args = get!(cursor.read_string(4));

                    serde_json::from_str(&args)
                        .ok()
                        .and_then(|json| {
                            match json {
                                Json::Array(array) => Some(array),
                                _ => None,
                            }
                        })
                        .map(|p| {
                            debug!("arguments: {:?}", p);
                            SlackerPacket::Request(SlackerRequest {
                                version: version,
                                serial_id: serial_id,
                                content_type: SlackerContentType::JSON,
                                fname: fname,
                                arguments: p,
                            })
                        })
                } else {
                    return None;
                }
            }
            2 => Some(SlackerPacket::Ping(SlackerPing { version: version })),
            7 => {
                if cursor.remaining() >= 3 {
                    let meta_type = get!(cursor.read_u8());
                    cursor.read_string(2).map(|s| {
                        SlackerPacket::InspectRequest(SlackerInspectRequest {
                            version: version,
                            serial_id: serial_id,
                            request_type: meta_type,
                            request_body: s,
                        })
                    })
                } else {
                    return None;
                }
            }
            _ => unimplemented!(),
        };
        (p, cursor.index)
    };

    buf.drain_to(index);
    packet
}

impl Codec for JsonSlackerCodec {
    type In = SlackerPacket<Json>;
    type Out = SlackerPacket<Json>;

    /// TODO: rewrite with nom
    fn decode(&mut self, buf: &mut EasyBuf) -> io::Result<Option<Self::Out>> {
        Ok(try_read_packet(buf))
    }

    fn encode(&mut self, frame: Self::In, buf: &mut Vec<u8>) -> io::Result<()> {
        match frame {
            SlackerPacket::Response(ref resp) => {
                debug!("writing version: {}", resp.version);
                try!(buf.write_u8(resp.version));
                debug!("writing serial id: {}", resp.serial_id);
                try!(buf.write_i32::<BigEndian>(resp.serial_id));
                // packet type: response, json, success
                try!(buf.write_all(&[1u8, 1u8, 0u8]));
                let serialized = serde_json::to_string(&resp.result).unwrap();
                debug!("writing serialized body: {}", serialized);
                try!(write_string(buf, &serialized, 4));
            }
            SlackerPacket::Error(ref resp) => {
                try!(buf.write_u8(resp.version));
                try!(buf.write_i32::<BigEndian>(resp.serial_id));
                // packet type: response, json, success
                try!(buf.write_all(&[4u8, resp.code]));
            }
            SlackerPacket::Pong(ref pong) => {
                try!(buf.write_u8(pong.version));
                try!(buf.write_i32::<BigEndian>(0));
            }
            SlackerPacket::InspectResponse(ref resp) => {
                try!(buf.write_u8(resp.version));
                try!(buf.write_i32::<BigEndian>(resp.serial_id));
                try!(write_string(buf, &resp.response_body, 2));
            }
            _ => unimplemented!(),

        }
        Ok(())
    }
    }
}
