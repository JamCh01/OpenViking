// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Conversational session lifecycle, message append, context, archive, and commit endpoints.

use http::Method;
use serde_json::{Map, Value};

use crate::client::AsyncHttpClient;
use crate::error::{JsonObject, Result};
use crate::message::Message;
use crate::options::{
    compact_request_body, merge_extra, BatchAddMessagesOptions, CommitSessionOptions,
    CreateSessionOptions, UpdateSessionConfigOptions,
};
use crate::transport::{encode_path_segment, QueryPairs};
use crate::types::Nullable;

/// Default token budget used by `get_session_context` (`128_000`).
pub const DEFAULT_SESSION_CONTEXT_TOKEN_BUDGET: usize = 128_000;

/// Stateful async handle bound to a single `session_id`, mirroring Python's `client.session(id)`.
#[derive(Debug, Clone)]
pub struct SessionHandle {
    client: AsyncHttpClient,
    session_id: String,
}

impl SessionHandle {
    pub(crate) fn new(client: AsyncHttpClient, session_id: String) -> Self {
        Self { client, session_id }
    }

    /// Returns the `session_id` bound to this handle.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Appends a single message to this session (`POST /api/v1/sessions/{id}/messages`).
    pub async fn add_message(&self, message: Message) -> Result<JsonObject> {
        self.client.add_message(&self.session_id, message).await
    }

    /// Appends a batch of messages to this session (`POST /api/v1/sessions/{id}/messages/batch`).
    pub async fn batch_add_messages(
        &self,
        messages: &[Message],
        options: BatchAddMessagesOptions,
    ) -> Result<JsonObject> {
        self.client
            .batch_add_messages(&self.session_id, messages, options)
            .await
    }

    /// Commits this session and triggers memory extraction (`POST /api/v1/sessions/{id}/commit`).
    pub async fn commit(&self, options: CommitSessionOptions) -> Result<JsonObject> {
        self.client.commit_session(&self.session_id, options).await
    }

    /// Deletes this session (`DELETE /api/v1/sessions/{id}`).
    pub async fn delete(&self) -> Result<()> {
        self.client.delete_session(&self.session_id).await
    }

    /// Loads metadata and messages for this session (`GET /api/v1/sessions/{id}`).
    pub async fn load(&self) -> Result<JsonObject> {
        self.client.get_session(&self.session_id, false).await
    }

    /// Assembles session context within `token_budget` (`GET /api/v1/sessions/{id}/context`).
    pub async fn get_session_context(&self, token_budget: usize) -> Result<JsonObject> {
        self.client
            .get_session_context(&self.session_id, token_budget)
            .await
    }

    /// Retrieves a completed session archive (`GET /api/v1/sessions/{id}/archives/{archive_id}`).
    pub async fn get_archive(&self, archive_id: &str) -> Result<JsonObject> {
        self.client
            .get_session_archive(&self.session_id, archive_id)
            .await
    }
}

impl AsyncHttpClient {
    /// Creates a new session (`POST /api/v1/sessions`).
    pub async fn create_session(&self, options: CreateSessionOptions) -> Result<JsonObject> {
        let mut payload = Map::new();
        if let Some(session_id) = &options.session_id {
            payload.insert("session_id".to_owned(), Value::String(session_id.clone()));
        }
        if let Some(policy) = &options.memory_policy {
            payload.insert("memory_policy".to_owned(), Value::Object(policy.clone()));
        }
        if let Some(extraction) = &options.memory_extraction_config {
            payload.insert(
                "memory_extraction_config".to_owned(),
                Value::Object(extraction.clone()),
            );
        }
        if let Some(telemetry) = &options.telemetry {
            payload.insert("telemetry".to_owned(), telemetry.clone());
        }
        merge_extra(
            &mut payload,
            &options.extra,
            &[
                "session_id",
                "memory_policy",
                "auto_commit_policy",
                "memory_extraction_config",
            ],
        )?;
        compact_request_body(&mut payload);

        // Preserve explicit JSON `null` after compaction so callers can disable server auto-commit.
        match options.auto_commit_policy {
            Nullable::Unset => {}
            Nullable::Null => {
                payload.insert("auto_commit_policy".to_owned(), Value::Null);
            }
            Nullable::Value(policy) => {
                payload.insert("auto_commit_policy".to_owned(), Value::Object(policy));
            }
        }

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/sessions",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Lists sessions visible to the caller (`GET /api/v1/sessions`).
    pub async fn list_sessions(&self) -> Result<Vec<Value>> {
        self.transport
            .request_result(Method::GET, "/api/v1/sessions", &[], None)
            .await
    }

    /// Gets one session (`GET /api/v1/sessions/{session_id}`).
    pub async fn get_session(&self, session_id: &str, auto_create: bool) -> Result<JsonObject> {
        let mut query: QueryPairs = Vec::new();
        if auto_create {
            query.push(("auto_create".to_owned(), "true".to_owned()));
        }
        let encoded = encode_path_segment(session_id);
        self.transport
            .request_result(
                Method::GET,
                &format!("/api/v1/sessions/{encoded}"),
                &query,
                None,
            )
            .await
    }

    /// Updates mutable session settings (`PATCH /api/v1/sessions/{session_id}/config`).
    pub async fn update_session_config(
        &self,
        session_id: &str,
        options: UpdateSessionConfigOptions,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        if let Some(extraction) = &options.memory_extraction_config {
            payload.insert(
                "memory_extraction_config".to_owned(),
                Value::Object(extraction.clone()),
            );
        }
        if let Some(telemetry) = &options.telemetry {
            payload.insert("telemetry".to_owned(), telemetry.clone());
        }
        merge_extra(
            &mut payload,
            &options.extra,
            &["auto_commit_policy", "memory_extraction_config"],
        )?;
        compact_request_body(&mut payload);

        match options.auto_commit_policy {
            Nullable::Unset => {}
            Nullable::Null => {
                payload.insert("auto_commit_policy".to_owned(), Value::Null);
            }
            Nullable::Value(policy) => {
                payload.insert("auto_commit_policy".to_owned(), Value::Object(policy));
            }
        }

        let encoded = encode_path_segment(session_id);
        self.transport
            .request_result(
                Method::PATCH,
                &format!("/api/v1/sessions/{encoded}/config"),
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Checks whether `session_id` exists (`false` on `NOT_FOUND` / HTTP 404).
    pub async fn session_exists(&self, session_id: &str) -> Result<bool> {
        match self.get_session(session_id, false).await {
            Ok(_) => Ok(true),
            Err(err) if err.is_not_found() => Ok(false),
            Err(err) => Err(err),
        }
    }

    /// Assembles session context within `token_budget` (`GET /api/v1/sessions/{id}/context`).
    pub async fn get_session_context(
        &self,
        session_id: &str,
        token_budget: usize,
    ) -> Result<JsonObject> {
        let query: QueryPairs = vec![("token_budget".to_owned(), token_budget.to_string())];
        let encoded = encode_path_segment(session_id);
        self.transport
            .request_result(
                Method::GET,
                &format!("/api/v1/sessions/{encoded}/context"),
                &query,
                None,
            )
            .await
    }

    /// Retrieves one committed session archive (`GET /api/v1/sessions/{id}/archives/{archive_id}`).
    pub async fn get_session_archive(
        &self,
        session_id: &str,
        archive_id: &str,
    ) -> Result<JsonObject> {
        let session_path = encode_path_segment(session_id);
        let archive_path = encode_path_segment(archive_id);
        self.transport
            .request_result(
                Method::GET,
                &format!("/api/v1/sessions/{session_path}/archives/{archive_path}"),
                &[],
                None,
            )
            .await
    }

    /// Deletes a session (`DELETE /api/v1/sessions/{id}`).
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        let encoded = encode_path_segment(session_id);
        self.transport
            .request_unit(
                Method::DELETE,
                &format!("/api/v1/sessions/{encoded}"),
                &[],
                None,
            )
            .await
    }

    /// Appends one message to `session_id` (`POST /api/v1/sessions/{id}/messages`).
    pub async fn add_message(&self, session_id: &str, message: Message) -> Result<JsonObject> {
        let wire = message.to_wire_object()?;
        let encoded = encode_path_segment(session_id);
        self.transport
            .request_result(
                Method::POST,
                &format!("/api/v1/sessions/{encoded}/messages"),
                &[],
                Some(&Value::Object(wire)),
            )
            .await
    }

    /// Appends multiple messages in a single request (`POST /api/v1/sessions/{id}/messages/batch`).
    pub async fn batch_add_messages(
        &self,
        session_id: &str,
        messages: &[Message],
        options: BatchAddMessagesOptions,
    ) -> Result<JsonObject> {
        let mut normalized = Vec::with_capacity(messages.len());
        for msg in messages {
            normalized.push(Value::Object(msg.to_wire_object()?));
        }

        let mut payload = Map::new();
        payload.insert("messages".to_owned(), Value::Array(normalized));
        if let Some(telemetry) = &options.telemetry {
            payload.insert("telemetry".to_owned(), telemetry.clone());
        }
        merge_extra(&mut payload, &options.extra, &["messages"])?;
        compact_request_body(&mut payload);

        let encoded = encode_path_segment(session_id);
        self.transport
            .request_result(
                Method::POST,
                &format!("/api/v1/sessions/{encoded}/messages/batch"),
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Commits a session and triggers memory extraction (`POST /api/v1/sessions/{id}/commit`).
    pub async fn commit_session(
        &self,
        session_id: &str,
        options: CommitSessionOptions,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert(
            "keep_recent_count".to_owned(),
            Value::Number(options.keep_recent_count.into()),
        );
        if let Some(retention_mode) = options.retention_mode {
            payload.insert("retention_mode".to_owned(), Value::String(retention_mode));
        }
        if let Some(turns) = options.keep_recent_turn_count {
            payload.insert(
                "keep_recent_turn_count".to_owned(),
                Value::Number(turns.into()),
            );
        }
        if let Some(budget) = options.retained_message_token_budget {
            payload.insert(
                "retained_message_token_budget".to_owned(),
                Value::Number(budget.into()),
            );
        }
        if let Some(min_steps) = options.min_raw_tail_steps {
            payload.insert(
                "min_raw_tail_steps".to_owned(),
                Value::Number(min_steps.into()),
            );
        }
        if let Some(telemetry) = options.telemetry {
            payload.insert("telemetry".to_owned(), telemetry);
        }
        if let Some(event_tags) = options.event_tags {
            let mut event_obj = Map::new();
            event_obj.insert(
                "tags".to_owned(),
                Value::Array(event_tags.into_iter().map(Value::String).collect()),
            );
            let mut extraction = Map::new();
            extraction.insert("event".to_owned(), Value::Object(event_obj));
            payload.insert("extraction_metadata".to_owned(), Value::Object(extraction));
        }
        merge_extra(
            &mut payload,
            &options.extra,
            &[
                "keep_recent_count",
                "enable_working_memory",
                "extraction_metadata",
            ],
        )?;
        compact_request_body(&mut payload);

        if let Some(working_mem) = options.enable_working_memory {
            payload.insert(
                "enable_working_memory".to_owned(),
                match working_mem {
                    Some(enabled) => Value::Bool(enabled),
                    None => Value::Null,
                },
            );
        }

        let encoded = encode_path_segment(session_id);
        self.transport
            .request_result(
                Method::POST,
                &format!("/api/v1/sessions/{encoded}/commit"),
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }
}
