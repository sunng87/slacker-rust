use nom::{IResult, Offset};
use bytes::{BytesMut, BufMut, Writer};
use tio::codec::{Encoder, Decoder};
use tproto::multiplex::RequestId;
use byteorder::{BigEndian, WriteBytesExt};

use std::io::{self, Write, ErrorKind};

//use packets::*;
use parser::*;

#[derive(Copy, Clone)]
pub struct SlackerCodec;

fn write_bytes(cur: &mut Writer<&mut BytesMut>, v: &[u8], prefix_len: usize) -> io::Result<()> {
    if prefix_len == 2 {
        try!(cur.write_u16::<BigEndian>(v.len() as u16));
    } else {
        try!(cur.write_u32::<BigEndian>(v.len() as u32));
    }

    cur.write_all(v)
}

fn write_string(cur: &mut Writer<&mut BytesMut>, v: &str, prefix_len: usize) -> io::Result<()> {
    write_bytes(cur, v.as_bytes(), prefix_len)
}

impl Encoder for SlackerCodec {
    type Item = (RequestId, SlackerPacket);
    type Error = io::Error;

    fn encode<'a>(&mut self, frame_in: Self::Item, buf0: &mut BytesMut) -> Result<(), Self::Error> {
        debug!("writing: {:?}", frame_in);
        let (_, SlackerPacket(header, body)) = frame_in;
        let mut buf = buf0.writer();
        try!(buf.write_u8(header.version));
        try!(buf.write_i32::<BigEndian>(header.serial_id));
        try!(buf.write_u8(header.packet_type));

        match body {
            SlackerPacketBody::Request(ref req) => {
                try!(buf.write_u8(req.content_type));
                try!(write_string(&mut buf, &req.fname, 2));
                try!(write_bytes(&mut buf, &req.arguments, 4));
            }
            SlackerPacketBody::Response(ref resp) => {
                try!(buf.write_u8(resp.content_type));
                try!(buf.write_u8(resp.result_code));
                try!(write_bytes(&mut buf, &resp.data, 4));
            }
            SlackerPacketBody::Error(ref resp) => {
                try!(buf.write_u8(resp.result_code));
            }
            SlackerPacketBody::Ping | SlackerPacketBody::Pong => {}
            SlackerPacketBody::InspectRequest(ref req) => {
                try!(buf.write_u8(req.inspect_type));
                try!(write_bytes(&mut buf, &req.data, 2));
            }
            SlackerPacketBody::InspectResponse(ref resp) => {
                try!(write_bytes(&mut buf, &resp.data, 2));
            }
            SlackerPacketBody::Interrupt(ref req) => {
                try!(buf.write_i32::<BigEndian>(req.req_id));
            }
        }
        Ok(())
    }
}

impl Decoder for SlackerCodec {
    type Item = (RequestId, SlackerPacket);
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let (consumed, result) = match slacker_all(buf.as_ref()) {
            IResult::Done(i, out) => {
                let SlackerPacket(header, _) = out;
                debug!("data in {:?}", header);
                let request_id = header.serial_id;

                (buf.offset(i), Some((request_id as RequestId, out)))
            }
            IResult::Incomplete(_) => return Ok(None),
            IResult::Error(e) => return Err(io::Error::new(ErrorKind::InvalidData, e)),
        };

        buf.split_to(consumed);
        Ok(result)
    }
}
