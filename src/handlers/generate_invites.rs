use std::sync::Arc;

use anyhow::Context;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::Response;
use serde::{Deserialize, Serialize};

use crate::handlers::shared::{ContentType, ServerSuccessResponse, success_response};
use crate::info;
use crate::model::database::db::Database;
use crate::model::repository::invites_repository;

#[derive(Serialize, Deserialize)]
pub struct GenerateInvitesRequest {
    pub amount_to_generate: u8
}

#[derive(Serialize, Deserialize)]
pub struct GenerateInvitesResponse {
    pub invites: Vec<String>
}

impl ServerSuccessResponse for GenerateInvitesResponse {

}

pub async fn handle(
    _query: &str,
    body: Incoming,
    database: &Arc<Database>,
    host_address: &String
) -> anyhow::Result<Response<Full<Bytes>>> {
    let body_bytes = body.collect()
        .await
        .context("Failed to collect body")?
        .to_bytes();

    let body_as_string = String::from_utf8(body_bytes.to_vec())
        .context("Failed to convert body into a string")?;

    let request: GenerateInvitesRequest = serde_json::from_str(body_as_string.as_str())
        .context("Failed to convert body into GenerateInvitesRequest")?;

    let generated_invites = invites_repository::generate_invites(
        database,
        request.amount_to_generate
    ).await?;

    let generated_invites_count = generated_invites.len();

    let generate_invites_response = GenerateInvitesResponse {
        invites: format_invites(host_address, generated_invites)
    };

    let response = Response::builder()
        .json()
        .status(200)
        .body(Full::new(Bytes::from(success_response(generate_invites_response)?)))?;

    info!("generate_invites() Success. Generated {} invites", generated_invites_count);
    return Ok(response);
}

fn format_invites(host_address: &String, generated_invites: Vec<String>) -> Vec<String> {
    return generated_invites
        .iter()
        .map(|invite_id| {
            return format!("{}/view_invite?invite={}", host_address, invite_id);
        })
        .collect::<Vec<String>>();
}