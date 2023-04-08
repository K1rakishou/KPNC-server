use hyper::http::response::Builder;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ServerResponse<T> {
    data: Option<T>,
    error: Option<String>
}

#[derive(Serialize, Deserialize)]
pub struct DefaultSuccessResponse {
    success: bool
}

#[derive(Serialize, Deserialize)]
struct EmptyField {

}

pub fn empty_success_response() -> anyhow::Result<String> {
    let response = ServerResponse {
        data: Some(DefaultSuccessResponse { success: true }),
        error: None
    };

    let json = serde_json::to_string(&response)?;
    return Ok(json);
}

pub fn success_response<'a, T>(
    data: T
) -> anyhow::Result<String>
    where T : Serialize
{
    let response = ServerResponse {
        data: Some(data),
        error: None
    };

    let json = serde_json::to_string(&response)?;
    return Ok(json);
}

pub fn error_response_string(error: &String) -> anyhow::Result<String> {
    return error_response(error.as_str());
}

pub fn error_response(error: &str) -> anyhow::Result<String> {
    let response: ServerResponse<EmptyField> = ServerResponse {
        data: None,
        error: Some(error.to_string())
    };

    let json = serde_json::to_string(&response)?;
    return Ok(json);
}

pub trait ContentType {
    fn content_type(self, value: &str) -> Builder;
    fn json(self) -> Builder;
}

impl ContentType for Builder {
    fn content_type(self, value: &str) -> Builder {
        return self.header("Content-Type", value)
    }

    fn json(self) -> Builder {
        return self.content_type("application/json")
    }
}