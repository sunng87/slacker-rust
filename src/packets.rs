pub static PROTOCOL_VERSION: u8 = 5;
pub static RESULT_CODE_SUCCESS: u8 = 0;
pub static RESULT_CODE_NOT_FOUND: u8 = 11;

#[derive(Debug)]
pub enum SlackerContentType {
    JSON,
}

#[derive(Debug)]
pub struct SlackerRequest<T>
    where T: Send + Sync + 'static
{
    pub version: u8,
    pub serial_id: i32,
    pub content_type: SlackerContentType,
    pub fname: String,
    pub arguments: Vec<T>,
}

#[derive(Debug)]
pub struct SlackerResponse<T>
    where T: Send + Sync + 'static
{
    pub version: u8,
    pub serial_id: i32,
    pub content_type: SlackerContentType,
    pub code: u8,
    pub result: T,
}

#[derive(Debug)]
pub struct SlackerInspectRequest {
    pub version: u8,
    pub serial_id: i32,
    pub request_type: u8,
    pub request_body: String,
}

#[derive(Debug)]
pub struct SlackerInspectResponse {
    pub version: u8,
    pub serial_id: i32,
    pub response_body: String,
}

#[derive(Debug)]
pub struct SlackerError {
    pub version: u8,
    pub serial_id: i32,
    pub code: u8,
}

#[derive(Debug)]
pub struct SlackerPing {
    pub version: u8,
}

#[derive(Debug)]
pub struct SlackerPong {
    pub version: u8,
}

#[derive(Debug)]
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
