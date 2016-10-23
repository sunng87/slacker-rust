use nom::{be_u8, be_u16, be_u32, be_i32, IResult};

#[derive(Debug)]
pub struct SlackerPacketHeader {
    pub version: u8,
    pub serial_id: i32,
    pub packet_type: u8,
}

named!(parse_slacker_header<SlackerPacketHeader>,
       chain!( v: be_u8 ~
               t: be_i32 ~
               p: be_u8 , || {
                   SlackerPacketHeader {
                       version: v,
                       serial_id: t,
                       packet_type: p
                   }
               }));

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
    pub result_code: u8
}


#[derive(Debug)]
pub struct SlackerInspectRequestPacket<'a> {
    pub inspect_type: u8,
    pub data: &'a str
}


#[derive(Debug)]
pub struct SlackerInspectResponsePacket<'a> {
    pub data: &'a str
}

   
#[derive(Debug)]
pub struct SlackerInterruptPacket {
    pub req_id: i32
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
    Pong
}


named!(parse_slacker_request<SlackerPacketBody>,
       chain!( ct: be_u8 ~
               fname_len: be_u16 ~
               fname: take_str!(fname_len) ~
               args_len: be_u32 ~
               args: take!(args_len) , || {
                   SlackerPacketBody::Request(
                       SlackerRequestPacket {
                           content_type: ct,
                           fname: fname,
                           arguments: args
                       })
               }));


named!(parse_slacker_response<SlackerPacketBody>,
       chain!(ct: be_u8 ~
              rt: be_u8 ~
              data_len: be_u32 ~
              data: take!(data_len), || {
                  SlackerPacketBody::Response(
                      SlackerResponsePacket {
                          content_type: ct,
                          result_code: rt,
                          data: data
                      })
              }));

named!(parse_slacker_error<SlackerPacketBody>,
       chain!(rt: be_u8, || {
           SlackerPacketBody::Error(
               SlackerErrorPacket {
                   result_code: rt
               })
       }));

named!(parse_slacker_inspect_req<SlackerPacketBody>,
       chain!(it: be_u8 ~
              data_len: be_u16 ~
              data: take_str!(data_len), || {
                  SlackerPacketBody::InspectRequest(
                      SlackerInspectRequestPacket {
                          inspect_type: it,
                          data: data
                      })
              }));

named!(parse_slacker_inspect_resp<SlackerPacketBody>,
       chain!(data_len: be_u16 ~
              data: take_str!(data_len), || {
                  SlackerPacketBody::InspectResponse(
                      SlackerInspectResponsePacket {
                          data: data
                      })
              }));

named!(parse_slacker_interrupt<SlackerPacketBody>,
       chain!(req_id: be_i32, || {
           SlackerPacketBody::Interrupt(
               SlackerInterruptPacket {
                   req_id: req_id
               })
       }));

pub fn parse_slacker_body(i: &[u8], hdr: SlackerPacketHeader) -> IResult<&[u8], SlackerPacketBody> {
    chain!(i,
           body: switch!(value!(hdr.packet_type),
                         0 => call!(parse_slacker_request) |
                         1 => call!(parse_slacker_response) | 
                         2 => value!(SlackerPacketBody::Ping) |
                         3 => value!(SlackerPacketBody::Pong) |
                         4 => call!(parse_slacker_error) |
                         7 => call!(parse_slacker_inspect_req) |
                         8 => call!(parse_slacker_inspect_resp) |
                         9 => call!(parse_slacker_interrupt)),
           || { body }
    )
}
