use bytes::{BytesMut, BufMut, Writer};
use tio::codec::{Encoder, Decoder};
use tproto::multiplex::RequestId;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde_json;
use serde_json::value::Value as Json;

use std::io::{self, Read, Write};

use packets::*;

#[derive(Copy, Clone)]
pub struct JsonSlackerCodec;


fn write_string(cur: &mut Writer<&mut BytesMut>, v: &str, prefix_len: usize) -> io::Result<()> {
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
    fn new(input: &'a BytesMut) -> EasyBufCursor<'a> {
        EasyBufCursor {
            buf: input.as_ref(),
            index: 0,
        }
    }

    fn remaining(&self) -> usize {
        self.buf.len()
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
        if self.remaining() > prefix_len {
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

fn try_read_packet(buf: &mut BytesMut) -> Option<(RequestId, SlackerPacket<Json>)> {
    if buf.len() < 6 {
        return None;
    }

    let (packet, req_id, index) = {
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
            1 => {
                if cursor.remaining() >= 6 {
                    // content type
                    let ct = get!(cursor.read_u8());
                    debug!("content-type {}", ct);
                    // result code
                    let rt = get!(cursor.read_u8());
                    // body
                    let body = get!(cursor.read_string(4));
                    serde_json::from_str(&body).ok().map(|r| {
                        debug!("result: {:?}", body);
                        SlackerPacket::Response(SlackerResponse {
                            version: version,
                            serial_id: serial_id,
                            content_type: SlackerContentType::JSON,
                            code: rt,
                            result: r,
                        })
                    })
                } else {
                    None
                }
            }
            2 => Some(SlackerPacket::Ping(SlackerPing { version: version })),
            3 => Some(SlackerPacket::Pong(SlackerPong { version: version })),
            4 => {
                cursor.read_u8().map(|rt| {
                    SlackerPacket::Error(SlackerError {
                        version: version,
                        serial_id: serial_id,
                        code: rt,
                    })
                })
            }
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
            8 => {
                cursor.read_string(2).map(|s| {
                    SlackerPacket::InspectResponse(SlackerInspectResponse {
                        version: version,
                        serial_id: serial_id,
                        response_body: s,
                    })
                })
            }

            _ => unimplemented!(),
        };
        (p, serial_id, cursor.index)
    };

    buf.split_to(index);
    packet.map(|p| (req_id as RequestId, p))

}

impl Encoder for JsonSlackerCodec {
    type Item = (RequestId, SlackerPacket<Json>);
    type Error = io::Error;

    fn encode(&mut self, frame_in: Self::Item, buf0: &mut BytesMut) -> Result<(), Self::Error> {
        let (_, frame) = frame_in;
        let mut buf = buf0.writer();
        match frame {
            SlackerPacket::Request(ref req) => {
                try!(buf.write_u8(req.version));
                try!(buf.write_i32::<BigEndian>(req.serial_id));
                // packet type: request, json
                try!(buf.write_all(&[0u8, 1u8]));
                try!(write_string(&mut buf, &req.fname, 2));
                let serialized = serde_json::to_string(&req.arguments).unwrap();
                try!(write_string(&mut buf, &serialized, 4));
            }
            SlackerPacket::Response(ref resp) => {
                debug!("writing version: {}", resp.version);
                try!(buf.write_u8(resp.version));
                debug!("writing serial id: {}", resp.serial_id);
                try!(buf.write_i32::<BigEndian>(resp.serial_id));
                // packet type: response, json, success
                try!(buf.write_all(&[1u8, 1u8, 0u8]));
                let serialized = serde_json::to_string(&resp.result).unwrap();
                debug!("writing serialized body: {}", serialized);
                try!(write_string(&mut buf, &serialized, 4));
            }
            SlackerPacket::Error(ref resp) => {
                try!(buf.write_u8(resp.version));
                try!(buf.write_i32::<BigEndian>(resp.serial_id));
                // packet type: response, json, success
                try!(buf.write_all(&[4u8, resp.code]));
            }
            SlackerPacket::Ping(ref ping) => {
                try!(buf.write_u8(ping.version));
                try!(buf.write_u8(2));
                try!(buf.write_i32::<BigEndian>(0));
            }
            SlackerPacket::Pong(ref pong) => {
                try!(buf.write_u8(pong.version));
                try!(buf.write_u8(3));
                try!(buf.write_i32::<BigEndian>(0));
            }
            SlackerPacket::InspectRequest(ref req) => {
                try!(buf.write_u8(req.version));
                try!(buf.write_u8(7));
                try!(buf.write_i32::<BigEndian>(req.serial_id));
                try!(buf.write_u8(req.request_type));
                try!(write_string(&mut buf, &req.request_body, 2));
            }
            SlackerPacket::InspectResponse(ref resp) => {
                try!(buf.write_u8(resp.version));
                try!(buf.write_i32::<BigEndian>(resp.serial_id));
                try!(write_string(&mut buf, &resp.response_body, 2));
            }
        }
        Ok(())
    }
}

impl Decoder for JsonSlackerCodec {
    type Item = (RequestId, SlackerPacket<Json>);
    type Error = io::Error;

    /// TODO: rewrite with nom
    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        Ok(try_read_packet(buf))
    }
}
