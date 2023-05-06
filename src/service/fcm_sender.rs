use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Context;
use fcm::Priority;
use lazy_static::lazy_static;
use serde::Serialize;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::{error, info};
use crate::model::database::db::Database;
use crate::model::repository::account_repository::AccountToken;
use crate::model::repository::post_reply_repository;
use crate::model::repository::post_reply_repository::UnsentReply;
use crate::model::repository::site_repository::SiteRepository;

lazy_static! {
    static ref FCM_CLIENT: fcm::Client = fcm::Client::new();
}

pub struct FcmSender {
    is_dev_build: bool,
    firebase_api_key: String,
    database: Arc<Database>,
    site_repository: Arc<SiteRepository>
}

#[derive(Debug, Serialize)]
struct NewFcmRepliesMessage {
    new_reply_messages: Vec<FcmReplyMessage>
}

#[derive(Debug, Serialize)]
struct FcmReplyMessage {
    reply_id: u64,
    new_reply_url: String
}

impl FcmSender {
    pub fn new(
        is_dev_build: bool,
        firebase_api_key: String,
        database: &Arc<Database>,
        site_repository: &Arc<SiteRepository>
    ) -> FcmSender {
        return FcmSender {
            is_dev_build,
            firebase_api_key,
            database: database.clone(),
            site_repository: site_repository.clone()
        };
    }

    pub async fn send_fcm_messages(&self, chunk_size: usize) -> anyhow::Result<()> {
        let unsent_replies = post_reply_repository::get_unsent_replies(
            self.is_dev_build,
            &self.database
        ).await.context("send_fcm_messages() Failed to get unsent replies")?;

        if unsent_replies.is_empty() {
            info!("send_fcm_messages() No unsent replies found");
            return Ok(());
        }

        for (firebase_token, unsent_replies_for_token) in &unsent_replies {
            info!(
                "send_fcm_messages() Got {} unsent replies for user with token {}",
                unsent_replies_for_token.len(),
                firebase_token
            );
        }

        let firebase_api_key = Arc::new(self.firebase_api_key.clone());
        let capacity = unsent_replies.len() / 2;
        let sent_post_reply_ids_set =
            Arc::new(RwLock::new(HashSet::<i64>::with_capacity(capacity)));
        let failed_to_send_post_reply_ids_set =
            Arc::new(RwLock::new(HashSet::<i64>::with_capacity(capacity)));
        let mut join_handles: Vec<JoinHandle<()>> = Vec::with_capacity(chunk_size);
        let semaphore = Arc::new(tokio::sync::Semaphore::new(chunk_size));

        for (account_token, unsent_replies) in unsent_replies {
            if unsent_replies.is_empty() {
                continue;
            }

            let semaphore_permit = semaphore.clone().acquire_owned().await?;
            let successfully_sent_cloned = sent_post_reply_ids_set.clone();
            let failed_to_send_post_reply_ids_cloned = failed_to_send_post_reply_ids_set.clone();
            let firebase_api_key_cloned = firebase_api_key.clone();
            let account_token_cloned = account_token.clone();
            let site_repository_cloned = self.site_repository.clone();

            let join_handle = tokio::task::spawn(async move {
                let result = send_unsent_reply(
                    &FCM_CLIENT,
                    &firebase_api_key_cloned,
                    &account_token_cloned,
                    &unsent_replies,
                    &successfully_sent_cloned,
                    &failed_to_send_post_reply_ids_cloned,
                    &site_repository_cloned
                ).await;

                drop(semaphore_permit);
                result.unwrap();
            });

            join_handles.push(join_handle);
        }

        futures::future::join_all(join_handles).await;

        let sent_post_reply_ids = {
            let sent_post_reply_ids_locked = sent_post_reply_ids_set.read().await;
            let mut result_vec = Vec::<i64>::with_capacity(sent_post_reply_ids_locked.len());

            sent_post_reply_ids_locked
                .iter()
                .for_each(|reply_id| result_vec.push(*reply_id));

            result_vec
        };

        if sent_post_reply_ids.len() > 0 {
            post_reply_repository::increment_notification_delivery_attempt(
                &sent_post_reply_ids,
                &self.database
            )
                .await
                .with_context(|| {
                    return "send_fcm_messages() Failed to increment notification \
                        delivery attempt counter";
                })?;
        }

        {
            let sent_post_reply_ids_set = sent_post_reply_ids_set.read().await;
            let failed_to_send_post_reply_ids_set = failed_to_send_post_reply_ids_set.read().await;

            info!(
                "send_fcm_messages() Done! Sent: {}, Not sent: {}",
                sent_post_reply_ids_set.len(),
                failed_to_send_post_reply_ids_set.len()
            );
        }

        return Ok(());
    }
}

async fn send_unsent_reply(
    client: &fcm::Client,
    firebase_api_key: &String,
    account_token: &AccountToken,
    unsent_replies: &HashSet<UnsentReply>,
    successfully_sent: &Arc<RwLock<HashSet<i64>>>,
    failed_to_send: &Arc<RwLock<HashSet<i64>>>,
    site_repository: &Arc<SiteRepository>
) -> anyhow::Result<()> {
    let new_reply_messages: Vec<FcmReplyMessage> = convert_unsent_replies_to_fcm_messages(
        unsent_replies,
        site_repository
    );

    if new_reply_messages.is_empty() {
        info!(
            "send_unsent_reply({}) new_reply_messages is empty",
            account_token
        );

        return Ok(());
    }

    let new_fcm_replies_message = NewFcmRepliesMessage {
        new_reply_messages
    };

    info!(
        "send_unsent_reply({}) new_fcm_replies_message: {:?}",
        account_token,
        new_fcm_replies_message
    );

    let new_fcm_replies_message_json = serde_json::to_string(&new_fcm_replies_message)?;

    let mut map = HashMap::new();
    map.insert("message_body", new_fcm_replies_message_json);

    let mut builder = fcm::MessageBuilder::new(firebase_api_key.as_str(), account_token.token.as_str());
    builder
        .priority(Priority::High)
        .data(&map)?;

    let response = client.send(builder.finalize()).await?;

    let error = response.error;
    if error.is_some() {
        {
            let mut failed_to_send_locked = failed_to_send.write().await;
            unsent_replies
                .iter()
                .for_each(|unsent_reply| {
                    failed_to_send_locked.insert(unsent_reply.post_reply_id_generated);
                });
        }

        let error = error.unwrap();
        error!(
            "send_unsent_reply({}) Failed to send FCM messages because of error: {:?}",
            account_token,
            error
        );
    } else {
        {
            let mut successfully_sent_locked = successfully_sent.write().await;
            unsent_replies
                .iter()
                .for_each(|unsent_reply| {
                    successfully_sent_locked.insert(unsent_reply.post_reply_id_generated);
                });
        }

        info!(
            "send_unsent_reply({}) Successfully sent a batch of {} replies",
            account_token,
            unsent_replies.len(),
        );
    }

    return Ok(());
}

fn convert_unsent_replies_to_fcm_messages(unsent_replies: &HashSet<UnsentReply>, site_repository: &Arc<SiteRepository>) -> Vec<FcmReplyMessage> {
    unsent_replies
        .into_iter()
        .filter_map(|unsent_reply| {
            let post_url = site_repository.to_url(&unsent_reply.post_descriptor);
            if post_url.is_none() {
                return None;
            }

            let post_url = post_url.unwrap();

            let fcm_reply_message = FcmReplyMessage {
                reply_id: unsent_reply.post_reply_id_generated as u64,
                new_reply_url: post_url
            };

            return Some(fcm_reply_message);
        })
        .collect()
}