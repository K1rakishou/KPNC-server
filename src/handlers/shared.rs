use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SuccessResponse<T> {
    data: T
}

#[derive(Serialize, Deserialize)]
pub struct ErrorResponse {
    error: String
}

#[derive(Serialize, Deserialize)]
pub struct DefaultSuccessResponse {
    success: bool
}

pub fn empty_success_response() -> anyhow::Result<String> {
    let response = SuccessResponse { data: DefaultSuccessResponse { success: true } };
    let json = serde_json::to_string(&response)?;
    return Ok(json);
}

pub fn success_response<'a, T>(
    data: T
) -> anyhow::Result<String>
    where T : Serialize + Deserialize<'a>
{
    let response = SuccessResponse { data };
    let json = serde_json::to_string(&response)?;
    return Ok(json);
}

pub fn error_response(error: &str) -> anyhow::Result<String> {
    let response = ErrorResponse { error: error.to_string() };
    let json = serde_json::to_string(&response)?;
    return Ok(json);
}

