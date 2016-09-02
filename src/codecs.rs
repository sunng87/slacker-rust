use tproto::io::{Parse, Serialize};
use tproto::pipeline::Frame;
use bytes::{Buf, BlockBuf, BlockBufCursor, MutBuf};
use byteorder::BigEndian;
use rustc_serialize::json::{self, Json};

use std::io;

use packets::*;

#[derive(Copy, Clone)]
pub struct JsonSlackerCodec;
fn read_string(cur: &mut BlockBufCursor, prefix_len: usize) -> Option<String> {
    if cur.remaining() >= prefix_len {
        let len: usize = if prefix_len == 2 {
            let raw = cur.read_u16::<BigEndian>();
            debug!("raw {}", raw);
            raw as usize
        } else {
            cur.read_u32::<BigEndian>() as usize
        };
        debug!("len: {}, remaining: {}", len, cur.remaining());
        if cur.remaining() >= len {
            let mut b = vec!(0u8; len);
            cur.read_slice(&mut b);
            String::from_utf8(b).ok()
        } else {
            None
        }
    } else {
        None
    }
}

fn write_string<W>(cur: &mut W, v: &str, prefix_len: usize)
    where W: MutBuf
{
    if prefix_len == 2 {
        cur.write_u16::<BigEndian>(v.len() as u16);
    } else {
        cur.write_u32::<BigEndian>(v.len() as u32);
    }

    cur.write_str(v);
}


fn try_read_packet(buf: &mut BlockBuf) -> Option<(Frame<SlackerPacket<Json>, io::Error>, usize)> {
    let mut cursor = buf.buf();
    let cur_rem = cursor.remaining();
    if cursor.remaining() < 6 {
        return None;
    }

    let version = cursor.read_u8();
    debug!("version {}", version);
    let serial_id = cursor.read_i32::<BigEndian>();
    debug!("serial id {}", serial_id);

    let packet_code = cursor.read_u8();
    debug!("packet code {}", packet_code);

    let p = match packet_code {
        0 => {
            if cursor.remaining() >= 3 {
                // content type
                let ct = cursor.read_u8();
                debug!("content-type {}", ct);

                let fname = read_string(&mut cursor, 2);
                if fname.is_none() {
                    return None;
                }
                let fname_string = fname.unwrap();
                debug!("fname {}", fname_string);

                let args = read_string(&mut cursor, 4);
                if args.is_none() {
                    return None;
                }
                let args_string = args.unwrap();
                debug!("args {}", args_string);

                Json::from_str(&args_string)
                    .ok()
                    .and_then(|json| {
                        match json {
                            Json::Array(array) => Some(array),
                            _ => None,
                        }
                    })
                    .map(|p| {
                        debug!("arguments: {:?}", p);
                        Frame::Message(SlackerPacket::Request(SlackerRequest {
                            version: version,
                            serial_id: serial_id,
                            content_type: SlackerContentType::JSON,
                            fname: fname_string,
                            arguments: p,
                        }))
                    })
            } else {
                return None;
            }
        }
        2 => Some(Frame::Message(SlackerPacket::Ping(SlackerPing { version: version }))),
        7 => {
            if cursor.remaining() >= 3 {
                let meta_type = cursor.read_u8();
                read_string(&mut cursor, 2).map(|s| {
                    Frame::Message(SlackerPacket::InspectRequest(SlackerInspectRequest {
                        version: version,
                        serial_id: serial_id,
                        request_type: meta_type,
                        request_body: s,
                    }))
                })
            } else {
                None
            }
        }
        _ => unimplemented!(),
    };

    p.map(|p| (p, cur_rem - cursor.remaining()))
}

impl Parse for JsonSlackerCodec {
    type Out = Frame<SlackerPacket<Json>, io::Error>;

    /// TODO: rewrite with nom
    fn parse(&mut self, buf: &mut BlockBuf) -> Option<Self::Out> {
        // Only compact if needed
        if !buf.is_compact() {
            buf.compact();
        }

        try_read_packet(buf).map(|(p, s)| {
            buf.shift(s);
            p
        })
    }

    fn done(&mut self, _: &mut BlockBuf) -> Option<Self::Out> {
        Some(Frame::Done)
    }
}


impl Serialize for JsonSlackerCodec {
    type In = Frame<SlackerPacket<Json>, io::Error>;

    fn serialize(&mut self, frame: Self::In, buf: &mut BlockBuf) {
        match frame {
            Frame::Message(packet) => {
                match packet {
                    SlackerPacket::Response(ref resp) => {
                        debug!("writing version: {}", resp.version);
                        buf.write_u8(resp.version);
                        debug!("writing serial id: {}", resp.serial_id);
                        buf.write_i32::<BigEndian>(resp.serial_id);
                        // packet type: response, json, success
                        buf.write_slice(&[1u8, 1u8, 0u8]);
                        let serialized = json::encode(&resp.result).unwrap();
                        debug!("writing serialized body: {}", serialized);
                        write_string(buf, &serialized, 4);
                    }
                    SlackerPacket::Error(ref resp) => {
                        buf.write_u8(resp.version);
                        buf.write_i32::<BigEndian>(resp.serial_id);
                        // packet type: response, json, success
                        buf.write_slice(&[4u8, resp.code]);
                    }
                    SlackerPacket::Pong(ref pong) => {
                        buf.write_u8(pong.version);
                        buf.write_i32::<BigEndian>(0);
                    }
                    SlackerPacket::InspectResponse(ref resp) => {
                        buf.write_u8(resp.version);
                        buf.write_i32::<BigEndian>(resp.serial_id);
                        write_string(buf, &resp.response_body, 2);
                    }
                    _ => unimplemented!(),
                }
            }
            _ => {}
        }
    }
}
