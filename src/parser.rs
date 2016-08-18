use nom::{be_u8, be_u16, be_u32};

#[derive(Debug)]
pub struct SlackerRequest<'a> {
    pub version: u8,
    pub serial_id: i32,
    pub content_type: u8,
    pub fname: String,
    pub arguments: &'a [u8],
}
