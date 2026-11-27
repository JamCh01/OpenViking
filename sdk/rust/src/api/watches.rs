// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Periodic resource watch management endpoints (`/api/v1/watches*`).

use http::Method;
use serde_json::{Map, Value};

use crate::client::AsyncHttpClient;
use crate::error::{JsonObject, OpenVikingError, Result};
use crate::options::{UpdateWatchOptions, WatchRef};
use crate::transport::{encode_path_segment, QueryPairs};
use crate::uri::VikingUri;

impl AsyncHttpClient {
    /// Lists watch tasks (`GET /api/v1/watches`).
    pub async fn list_watches(
        &self,
        active_only: bool,
        to_uri: Option<&str>,
    ) -> Result<JsonObject> {
        let mut query: QueryPairs = vec![("active_only".to_owned(), active_only.to_string())];
        if let Some(uri) = to_uri {
            query.push(("to_uri".to_owned(), VikingUri::normalize(uri)));
        }
        self.transport
            .request_result(Method::GET, "/api/v1/watches", &query, None)
            .await
    }

    /// Retrieves one watch task by `task_id` (`GET /api/v1/watches/{task_id}`).
    pub async fn get_watch(&self, task_id: &str, to_uri: Option<&str>) -> Result<JsonObject> {
        let mut query: QueryPairs = Vec::new();
        if let Some(uri) = to_uri {
            query.push(("to_uri".to_owned(), VikingUri::normalize(uri)));
        }
        let encoded = encode_path_segment(task_id);
        self.transport
            .request_result(
                Method::GET,
                &format!("/api/v1/watches/{encoded}"),
                &query,
                None,
            )
            .await
    }

    /// Partially updates a watch task by `task_id` or `to_uri` (`PATCH /api/v1/watches*`).
    pub async fn update_watch(&self, options: UpdateWatchOptions) -> Result<JsonObject> {
        let (path, query) = resolve_watch_path_and_query(&options.target, "")?;
        let mut payload = Map::new();
        if let Some(interval) = options.watch_interval {
            if let Some(num) = serde_json::Number::from_f64(interval) {
                payload.insert("watch_interval".to_owned(), Value::Number(num));
            }
        }
        if let Some(is_active) = options.is_active {
            payload.insert("is_active".to_owned(), Value::Bool(is_active));
        }
        if let Some(reason) = options.reason {
            payload.insert("reason".to_owned(), Value::String(reason));
        }
        if let Some(instruction) = options.instruction {
            payload.insert("instruction".to_owned(), Value::String(instruction));
        }

        self.transport
            .request_result(Method::PATCH, &path, &query, Some(&Value::Object(payload)))
            .await
    }

    /// Deletes a watch task by `task_id` or `to_uri` (`DELETE /api/v1/watches*`).
    pub async fn delete_watch(&self, target: WatchRef) -> Result<JsonObject> {
        let (path, query) = resolve_watch_path_and_query(&target, "")?;
        self.transport
            .request_result(Method::DELETE, &path, &query, None)
            .await
    }

    /// Triggers immediate background execution of a watch task (`POST /api/v1/watches*/trigger`).
    pub async fn trigger_watch(&self, target: WatchRef) -> Result<JsonObject> {
        let (path, query) = resolve_watch_path_and_query(&target, "/trigger")?;
        self.transport
            .request_result(Method::POST, &path, &query, None)
            .await
    }
}

fn resolve_watch_path_and_query(target: &WatchRef, suffix: &str) -> Result<(String, QueryPairs)> {
    let task_id = target.task_id.as_deref().filter(|s| !s.is_empty());
    let to_uri = target.to_uri.as_deref().filter(|s| !s.is_empty());
    if task_id.is_none() && to_uri.is_none() {
        return Err(OpenVikingError::invalid_argument(
            "Either task_id or to_uri is required",
        ));
    }

    let mut query: QueryPairs = Vec::new();
    if let Some(uri) = to_uri {
        query.push(("to_uri".to_owned(), VikingUri::normalize(uri)));
    }
    let path = match task_id {
        Some(id) => format!("/api/v1/watches/{}{suffix}", encode_path_segment(id)),
        None => format!("/api/v1/watches{suffix}"),
    };
    Ok((path, query))
}
