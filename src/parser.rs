use nom::{be_i32, be_u16, be_u32, be_u8};

pub const PROTOCOL_VERSION_5: u8 = 5;
pub const RESULT_CODE_SUCCESS: u8 = 0;
pub const RESULT_CODE_NOT_FOUND: u8 = 11;

pub const JSON_CONTENT_TYPE: u8 = 1;

pub const PACKET_TYPE_REQUEST: u8 = 0;
pub const PACKET_TYPE_RESPONSE: u8 = 1;
pub const PACKET_TYPE_PING: u8 = 2;
pub const PACKET_TYPE_PONG: u8 = 3;
pub const PACKET_TYPE_ERROR: u8 = 4;
pub const PACKET_TYPE_INSPECT_REQUEST: u8 = 7;
pub const PACKET_TYPE_INSPECT_RESPONSE: u8 = 8;
pub const PACKET_TYPE_INTERRUPT: u8 = 9;

#[derive(Debug, Copy, Clone)]
pub struct SlackerPacketHeader {
    pub version: u8,
    pub serial_id: i32,
    pub packet_type: u8,
}

named!(
    slacker_header<SlackerPacketHeader>,
    do_parse!(
        v: be_u8 >> t: be_i32 >> p: be_u8 >> (SlackerPacketHeader {
            version: v,
            serial_id: t,
            packet_type: p
        })
    )
);

#[derive(Debug)]
pub struct SlackerRequestPacket {
    pub content_type: u8,
    pub fname: String,
    pub arguments: Vec<u8>,
}

#[derive(Debug)]
pub struct SlackerResponsePacket {
    pub content_type: u8,
    pub result_code: u8,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct SlackerErrorPacket {
    pub result_code: u8,
}

#[derive(Debug)]
pub struct SlackerInspectRequestPacket {
    pub inspect_type: u8,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct SlackerInspectResponsePacket {
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct SlackerInterruptPacket {
    pub req_id: i32,
}

#[derive(Debug)]
pub enum SlackerPacketBody {
    Request(SlackerRequestPacket),
    Response(SlackerResponsePacket),
    Error(SlackerErrorPacket),
    InspectRequest(SlackerInspectRequestPacket),
    InspectResponse(SlackerInspectResponsePacket),
    Interrupt(SlackerInterruptPacket),
    Ping,
    Pong,
}

named!(slacker_request <&[u8], SlackerPacketBody>,
       do_parse!( ct: be_u8 >>
                  fname_len: be_u16 >>
                  fname: take_str!(fname_len) >>
                  args_len: be_u32 >>
                  args: take!(args_len) >>
                  (
                      SlackerPacketBody::Request(
                          SlackerRequestPacket {
                              content_type: ct,
                              fname: fname.to_owned(),
                              arguments: args.into()
                          }
                      )
                  )));

named!(slacker_response <&[u8], SlackerPacketBody>,
       do_parse!(ct: be_u8 >>
                 rt: be_u8 >>
                 data_len: be_u32 >>
                 data: take!(data_len) >>
                 (
                     SlackerPacketBody::Response(
                         SlackerResponsePacket {
                             content_type: ct,
                             result_code: rt,
                             data: data.into()
                         })
                 )));

named!(slacker_error <&[u8], SlackerPacketBody>,
       do_parse!(rt: be_u8 >>
                 (
                     SlackerPacketBody::Error(
                         SlackerErrorPacket {
                             result_code: rt
                         }
                     )
                 )
       ));

named!(slacker_inspect_req <&[u8], SlackerPacketBody>,
       do_parse!(it: be_u8 >>
                 data_len: be_u16 >>
                 data: take!(data_len) >>
                 (
                     SlackerPacketBody::InspectRequest(
                         SlackerInspectRequestPacket {
                             inspect_type: it,
                             data: data.into()
                         }
                     )
                 )));

named!(slacker_inspect_resp <&[u8], SlackerPacketBody>,
       do_parse!(data_len: be_u16 >>
                 data: take!(data_len) >>
                 (
                     SlackerPacketBody::InspectResponse(
                         SlackerInspectResponsePacket {
                             data: data.into()
                         })
                 )
       ));

named!(slacker_interrupt <&[u8], SlackerPacketBody>,
       do_parse!(req_id: be_i32 >>
                 (
                     SlackerPacketBody::Interrupt(
                         SlackerInterruptPacket {
                             req_id: req_id
                         })
                 )
       ));

#[derive(Debug)]
pub struct SlackerPacket(pub SlackerPacketHeader, pub SlackerPacketBody);

named!(pub slacker_all <&[u8], SlackerPacket>,
       do_parse!(header: slacker_header >>
                 body: switch!(value!(header.packet_type),
                               PACKET_TYPE_REQUEST => call!(slacker_request) |
                               PACKET_TYPE_RESPONSE => call!(slacker_response) |
                               PACKET_TYPE_PING => value!(SlackerPacketBody::Ping) |
                               PACKET_TYPE_PONG => value!(SlackerPacketBody::Pong) |
                               PACKET_TYPE_ERROR => call!(slacker_error) |
                               PACKET_TYPE_INSPECT_REQUEST => call!(slacker_inspect_req) |
                               PACKET_TYPE_INSPECT_RESPONSE => call!(slacker_inspect_resp) |
                               PACKET_TYPE_INTERRUPT => call!(slacker_interrupt)) >>
                 (SlackerPacket(header, body))
       ));
