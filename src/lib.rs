extern crate tokio;
extern crate futures;
extern crate rustc_serialize;
extern crate bytes;
extern crate byteorder;

use tokio::{server, Service, NewService};
use tokio::io::{Parse, Serialize, Framed};
use tokio::proto::pipeline::{Server, Frame};
use tokio::reactor::Reactor;
use bytes::{Buf, BlockBuf, BlockBufCursor, MutBuf};
use futures::{Future, finished, Promise, failed};
use rustc_serialize::json::{self, Json};
use byteorder::{BigEndian, ByteOrder};

use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;
use std::net::SocketAddr;

static PROTOCOL_VERSION: i16 = 5;
static RESULT_CODE_SUCCESS: u8 = 0;
static RESULT_CODE_NOT_FOUND: u8 = 11;

pub enum SlackerContentType {
    JSON,
}

pub struct SlackerRequest<T>
    where T: Send + Sync + 'static
{
    pub version: u8,
    pub serial_id: i32,
    pub content_type: SlackerContentType,
    pub fname: String,
    pub arguments: Vec<T>,
}

pub struct SlackerResponse<T>
    where T: Send + Sync + 'static
{
    pub version: u8,
    pub serial_id: i32,
    pub content_type: SlackerContentType,
    pub code: u8,
    pub result: T,
}

pub struct SlackerInspectRequest {
    pub version: u8,
    pub serial_id: i32,
    pub request_type: u8,
    pub request_body: String,
}

pub struct SlackerInspectResponse {
    pub version: u8,
    pub serial_id: i32,
    pub response_body: String,
}

pub struct SlackerError {
    pub version: u8,
    pub serial_id: i32,
    pub code: u8,
}

pub struct SlackerPing {
    pub version: u8,
}

pub struct SlackerPong {
    pub version: u8,
}

pub enum SlackerPacket<T>
    where T: Send + Sync + 'static
{
    Request(SlackerRequest<T>),
    Response(SlackerResponse<T>),
    Error(SlackerError),
    Ping(SlackerPing),
    Pong(SlackerPong),
    InspectRequest(SlackerInspectRequest),
    InspectResponse(SlackerInspectResponse),
}

#[derive(Clone)]
pub struct SlackerService<T>
    where T: Send + Sync + 'static
{
    functions: Arc<BTreeMap<String, Box<Fn(&Vec<T>) -> Promise<T> + Send + Sync + 'static>>>,
}

impl<T> SlackerService<T>
    where T: Send + Sync + 'static
{
    pub fn new(functions: BTreeMap<String, Box<Fn(&Vec<T>) -> Promise<T> + Send + Sync + 'static>>)
               -> SlackerService<T> {
        SlackerService { functions: Arc::new(functions) }
    }
}

impl<T> Service for SlackerService<T>
    where T: Send + Sync + 'static
{
    type Req = SlackerPacket<T>;
    type Resp = SlackerPacket<T>;
    type Error = io::Error;
    type Fut = Box<Future<Item = Self::Resp, Error = Self::Error>>;
    // type Fut = Finished<SlackerPacket<T>, ()>;

    fn call(&self, req: Self::Req) -> Self::Fut {
        match req {
            SlackerPacket::Request(sreq) => {
                if let Some(f) = self.functions.get(&sreq.fname) {
                    f(&sreq.arguments)
                        .and_then(move |result| {
                            finished(SlackerPacket::Response(SlackerResponse {
                                version: sreq.version,
                                code: RESULT_CODE_SUCCESS,
                                content_type: sreq.content_type,
                                serial_id: sreq.serial_id,
                                result: result,
                            }))
                        })
                        .map_err(|_| io::Error::new(io::ErrorKind::Other, "Promised cancelled"))
                        .boxed()
                } else {
                    let error = SlackerError {
                        version: sreq.version,
                        code: RESULT_CODE_NOT_FOUND,
                        serial_id: sreq.serial_id,
                    };
                    finished(SlackerPacket::Error(error)).boxed()
                }
            }
            SlackerPacket::Ping(ref ping) => {
                finished(SlackerPacket::Pong(SlackerPong { version: ping.version })).boxed()
            }
            _ => {
                Box::new(failed(io::Error::new(io::ErrorKind::InvalidInput, "Unsupported packet")))
            }
        }
    }
}

#[derive(Copy, Clone)]
pub struct JsonSlackerCodec;

fn read_i16(cur: &mut BlockBufCursor) -> i16 {
    let mut b = [0; 2];
    cur.read_slice(&mut b);
    BigEndian::read_i16(&b)
}

fn read_u16(cur: &mut BlockBufCursor) -> u16 {
    let mut b = [0; 2];
    cur.read_slice(&mut b);
    BigEndian::read_u16(&b)
}

fn read_i32(cur: &mut BlockBufCursor) -> i32 {
    let mut b = [0; 4];
    cur.read_slice(&mut b);
    BigEndian::read_i32(&b)
}

fn read_u32(cur: &mut BlockBufCursor) -> u32 {
    let mut b = [0; 4];
    cur.read_slice(&mut b);
    BigEndian::read_u32(&b)
}

fn write_i16<W>(cur: &mut W, v: i16)
    where W: MutBuf
{
    let mut b = [0; 2];
    BigEndian::write_i16(&mut b, v);
    cur.write_slice(&b);
}

fn write_u16<W>(cur: &mut W, v: u16)
    where W: MutBuf
{
    let mut b = [0; 2];
    BigEndian::write_u16(&mut b, v);
    cur.write_slice(&b);
}

fn write_i32<W>(cur: &mut W, v: i32)
    where W: MutBuf
{
    let mut b = [0; 4];
    BigEndian::write_i32(&mut b, v);
    cur.write_slice(&b);
}

fn write_u32<W>(cur: &mut W, v: u32)
    where W: MutBuf
{
    let mut b = [0; 4];
    BigEndian::write_u32(&mut b, v);
    cur.write_slice(&b);
}

fn read_string(cur: &mut BlockBufCursor, prefix_len: usize) -> Option<String> {
    if cur.remaining() >= prefix_len {
        let len: usize = if prefix_len == 2 {
            read_u16(cur) as usize
        } else {
            read_u32(cur) as usize
        };
        if cur.remaining() >= len {
            let mut b = Vec::<u8>::new();
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
        write_u16(cur, v.len() as u16)
    } else {
        write_u32(cur, v.len() as u32)
    }

    cur.write_str(v);
}

impl Parse for JsonSlackerCodec {
    type Out = Frame<SlackerPacket<Json>, io::Error>;

    /// TODO: rewrite with nom
    fn parse(&mut self, buf: &mut BlockBuf) -> Option<Self::Out> {
        // Only compact if needed
        if !buf.is_compact() {
            buf.compact();
        }

        let mut cursor = buf.buf();
        if cursor.remaining() < 6 {
            return None;
        }


        let version = cursor.read_byte().unwrap();
        let serial_id = read_i32(&mut cursor);

        let packet_code = cursor.read_byte().unwrap();

        match packet_code {
            0 => {
                if cursor.remaining() >= 3 {
                    // content type
                    let _ = cursor.read_byte().unwrap();

                    let fname = read_string(&mut cursor, 2);
                    if fname.is_none() {
                        return None;
                    }

                    let args = read_string(&mut cursor, 4);
                    if args.is_none() {
                        return None;
                    }

                    Json::from_str(&args.unwrap())
                        .ok()
                        .and_then(|json| {
                            match json {
                                Json::Array(array) => Some(array),
                                _ => None,
                            }
                        })
                        .map(|p| {
                            Frame::Message(SlackerPacket::Request(SlackerRequest {
                                version: version,
                                serial_id: serial_id,
                                content_type: SlackerContentType::JSON,
                                fname: fname.unwrap(),
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
                    let meta_type = cursor.read_byte().unwrap();
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
        }
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
                        buf.write_slice(&[resp.version]);
                        write_i32(buf, resp.serial_id);
                        // packet type: response, json, success
                        buf.write_slice(&[1, 1, 0]);
                        let serialized = json::encode(&resp.result).unwrap();
                        write_string(buf, &serialized, 4);
                    }
                    SlackerPacket::Error(ref resp) => {
                        buf.write_slice(&[resp.version]);
                        write_i32(buf, resp.serial_id);
                        // packet type: response, json, success
                        buf.write_slice(&[4, resp.code]);
                    }
                    SlackerPacket::Pong(ref pong) => {
                        buf.write_slice(&[pong.version]);
                        write_i32(buf, 0);
                    }
                    SlackerPacket::InspectResponse(ref resp) => {
                        buf.write_slice(&[resp.version]);
                        write_i32(buf, resp.serial_id);
                        write_string(buf, &resp.response_body, 2);
                    }
                    _ => unimplemented!(),
                }
            }
            _ => {}
        }
    }
}


pub fn serve<T>(addr: SocketAddr, new_service: T)
    where T: NewService< Req = SlackerPacket<Json>, Resp = SlackerPacket<Json>, Error = io::Error> + Send + 'static {

    let reactor = Reactor::default().unwrap();
    let handle = reactor.handle();

    server::listen(&handle, addr, move |socket| {
        // Create the service
        let service = try!(new_service.new_service());

        // Create the transport
        let codec = JsonSlackerCodec;
        let transport = Framed::new(socket,
                                    codec,
                                    codec,
                                    BlockBuf::default(),
                                    BlockBuf::default());

        // Return the pipeline server task
        Server::new(service, transport)
    })
        .unwrap();

    reactor.run().unwrap();
}
