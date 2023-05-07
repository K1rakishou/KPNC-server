use anyhow::anyhow;
use lazy_static::lazy_static;
use serde::de::DeserializeOwned;

lazy_static! {
    static ref BASE_URL: String = String::from("http://127.0.0.1:3000");
    static ref HTTP_CLIENT: reqwest::Client = reqwest::Client::new();
}

pub async fn post_request<'a, Response : DeserializeOwned>(
    endpoint: &str,
    body: &String,
    master_password: &str,
) -> anyhow::Result<Response> {
    let full_url = format!("{}/{}", *BASE_URL, endpoint);

    let request = HTTP_CLIENT.post(full_url)
        .body(body.clone())
        .header("X-Master-Password", master_password.to_string())
        .build()?;

    let response = HTTP_CLIENT.execute(request).await.unwrap();

    let status = response.status().as_u16();
    if status != 200 {
        return Err(anyhow!("Bad response status: {}", status))
    }

    let text = response.text().await?;
    let response_data = serde_json::from_str::<Response>(&text)?;

    return Ok(response_data);
}