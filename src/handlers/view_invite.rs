use std::sync::Arc;

use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::Response;

use crate::handlers::shared::ContentType;
use crate::helpers::string_helpers;
use crate::helpers::string_helpers::query_to_params;
use crate::model::database::db::Database;
use crate::model::repository::invites_repository;
use crate::model::repository::invites_repository::NEW_ACCOUNT_TRIAL_PERIOD_DAYS;

pub async fn handle(
    query: &str,
    _: Incoming,
    database: &Arc<Database>,
    host_address: &String
) -> anyhow::Result<Response<Full<Bytes>>> {
    let params = query_to_params(query);

    let def = "".to_string();
    let invite = params.get("invite").unwrap_or(&def);
    if invite.is_empty() {
        return invite_parameter_is_empty();
    }

    let user_id = invites_repository::accept_invite(&invite, database).await?;
    if user_id.is_none() {
        return failed_to_accept_invite();
    }

    let user_id = user_id.unwrap();
    return success(&user_id, host_address, NEW_ACCOUNT_TRIAL_PERIOD_DAYS);
}

fn success(
    user_id: &String,
    host_address: &String,
    free_days_amount: usize
) -> anyhow::Result<Response<Full<Bytes>>> {
    let html = r#"
<html>
    <body>
        <h3>Invite accepted, do not reload the page or it will be lost!</h3>
        <div>
            Copy this user_id
            <br>
                <b><span style="color:red; font-size: 150%">{{user_id}}</span></b>
            <br>
            and store it somewhere. Use it to login into your account to be able to use push notifications.
            <br>
            <br>
            In KurobaExLite:
            <ul>
                <li>Go to Application settings</li>
                <li>Push notification settings</li>
                <li>Enter {{host_address}} into Instance address input</li>
                <li>Copy the user_id into UserId input</li>
                <li>Click login</li>
                <li>Use should be able to use push notifications for free for {{free_days_count}} days</li>
            </ul>
        </div>
    </body>
</html>
    "#;

    let user_id = string_helpers::insert_after_every_nth(&user_id, "<wbr>", 32);
    let html = html.replace("{{user_id}}", &user_id);
    let html = html.replace("{{host_address}}", &host_address);
    let html = html.replace("{{free_days_count}}", &free_days_amount.to_string());

    let response = Response::builder()
        .status(200)
        .html()
        .body(Full::new(Bytes::from(html)))?;

    return Ok(response)
}

fn failed_to_accept_invite() -> anyhow::Result<Response<Full<Bytes>>> {
    let html = r#"
<html>
    <body>
        <h3>Error while trying to accept invite</h3>
        <div>
            Failed to accept invite (doesn't exist or already expired)
        </div>
    </body>
</html>
    "#;

    let response = Response::builder()
        .status(200)
        .html()
        .body(Full::new(Bytes::from(html)))?;

    return Ok(response)
}

fn invite_parameter_is_empty() -> anyhow::Result<Response<Full<Bytes>>> {
    let html = r#"
<html>
    <body>
        <h3>Error while trying to accept invite</h3>
        <div>
            'invite' parameter is empty
        </div>
    </body>
</html>
    "#;

    let response = Response::builder()
        .status(200)
        .html()
        .body(Full::new(Bytes::from(html)))?;

    return Ok(response)
}