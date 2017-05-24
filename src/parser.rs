use nom::{be_u8, be_u16, be_u32, be_i32};

#[derive(Debug)]
pub struct SlackerPacketHeader {
    pub version: u8,
    pub serial_id: i32,
    pub packet_type: u8,
}

named!(slacker_header<SlackerPacketHeader>,
       do_parse!( v: be_u8 >>
                  t: be_i32 >>
                  p: be_u8 >>
                  (
                      SlackerPacketHeader {
                          version: v,
                          serial_id: t,
                          packet_type: p
                      }
                  )));

#[derive(Debug)]
pub struct SlackerRequestPacket<'a> {
    pub content_type: u8,
    pub fname: &'a str,
    pub arguments: &'a [u8],
}

#[derive(Debug)]
pub struct SlackerResponsePacket<'a> {
    pub content_type: u8,
    pub result_code: u8,
    pub data: &'a [u8],
}



#[derive(Debug)]
pub struct SlackerErrorPacket {
    pub result_code: u8,
}


#[derive(Debug)]
pub struct SlackerInspectRequestPacket<'a> {
    pub inspect_type: u8,
    pub data: &'a str,
}


#[derive(Debug)]
pub struct SlackerInspectResponsePacket<'a> {
    pub data: &'a str,
}


#[derive(Debug)]
pub struct SlackerInterruptPacket {
    pub req_id: i32,
}

#[derive(Debug)]
pub enum SlackerPacketBody<'a> {
    Request(SlackerRequestPacket<'a>),
    Response(SlackerResponsePacket<'a>),
    Error(SlackerErrorPacket),
    InspectRequest(SlackerInspectRequestPacket<'a>),
    InspectResponse(SlackerInspectResponsePacket<'a>),
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
                              fname: fname,
                              arguments: args
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
                             data: data
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
                 data: take_str!(data_len) >>
                 (
                     SlackerPacketBody::InspectRequest(
                         SlackerInspectRequestPacket {
                             inspect_type: it,
                             data: data
                         }
                     )
                 )));

named!(slacker_inspect_resp <&[u8], SlackerPacketBody>,
       do_parse!(data_len: be_u16 >>
                 data: take_str!(data_len) >>
                 (
                     SlackerPacketBody::InspectResponse(
                         SlackerInspectResponsePacket {
                             data: data
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

named!(slacker_all <&[u8], (SlackerPacketHeader, SlackerPacketBody)>,
       do_parse!(header: slacker_header >>
                 body: switch!(value!(header.packet_type),
                               0 => call!(slacker_request) |
                               1 => call!(slacker_response) |
                               2 => value!(SlackerPacketBody::Ping) |
                               3 => value!(SlackerPacketBody::Pong) |
                               4 => call!(slacker_error) |
                               7 => call!(slacker_inspect_req) |
                               8 => call!(slacker_inspect_resp) |
                               9 => call!(slacker_interrupt)) >>
                 ((header, body))
       ));
