// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! System health, consistency check, and observer status endpoints (`/health`, `/api/v1/system/*`, `/api/v1/observer/*`).

use http::Method;
use serde_json::{Map, Value};

use crate::client::AsyncHttpClient;
use crate::error::{JsonObject, Result};
use crate::transport::QueryPairs;
use crate::types::ObserverFormat;
use crate::uri::VikingUri;

/// Async observer status sub-namespace returned by [`AsyncHttpClient::observer`].
#[derive(Debug, Clone)]
pub struct ObserverNamespace {
    client: AsyncHttpClient,
}

impl ObserverNamespace {
    pub(crate) fn new(client: AsyncHttpClient) -> Self {
        Self { client }
    }

    /// Queries processing queue status (`GET /api/v1/observer/queue`).
    pub async fn queue(&self) -> Result<JsonObject> {
        self.client.queue_status(None).await
    }

    /// Queries VikingDB storage status (`GET /api/v1/observer/vikingdb`).
    pub async fn vikingdb(&self) -> Result<JsonObject> {
        self.client.vikingdb_status(None).await
    }

    /// Queries configured model status (`GET /api/v1/observer/models`).
    pub async fn models(&self) -> Result<JsonObject> {
        self.client.models_status(None).await
    }

    /// Queries aggregate system status (`GET /api/v1/observer/system`).
    pub async fn system(&self) -> Result<JsonObject> {
        self.client.get_status(None).await
    }

    /// Reports whether `observer/system` reports `is_healthy == true`.
    pub async fn is_healthy(&self) -> Result<bool> {
        self.client.is_healthy().await
    }
}

impl AsyncHttpClient {
    /// Waits for queued background processing tasks to drain (`POST /api/v1/system/wait`).
    pub async fn wait_processed(&self) -> Result<JsonObject> {
        let empty = Value::Object(Map::new());
        self.transport
            .request_result(Method::POST, "/api/v1/system/wait", &[], Some(&empty))
            .await
    }

    /// Checks filesystem/vector-index consistency under `uri` (`POST /api/v1/system/consistency`).
    pub async fn check_consistency(&self, uri: &str) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("uri".to_owned(), Value::String(VikingUri::normalize(uri)));
        self.transport
            .request_result(
                Method::POST,
                "/api/v1/system/consistency",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Probes `GET /health`, returning `Ok(true)` when the server responds with `{"status": "ok"}`
    /// and `Ok(false)` on HTTP or JSON errors (matching Python `client.health()`).
    pub async fn health(&self) -> bool {
        let Ok(response) = self
            .transport
            .send_raw(Method::GET, "/health", &[], None)
            .await
        else {
            return false;
        };
        if !response.status().is_success() {
            return false;
        }
        let Ok(bytes) = response.bytes().await else {
            return false;
        };
        let Ok(val) = serde_json::from_slice::<Value>(&bytes) else {
            return false;
        };
        val.get("status").and_then(Value::as_str) == Some("ok")
    }

    /// Returns observer queue status (`GET /api/v1/observer/queue`).
    pub async fn queue_status(&self, format: Option<ObserverFormat>) -> Result<JsonObject> {
        self.observer_get("/api/v1/observer/queue", format).await
    }

    /// Returns observer VikingDB status (`GET /api/v1/observer/vikingdb`).
    pub async fn vikingdb_status(&self, format: Option<ObserverFormat>) -> Result<JsonObject> {
        self.observer_get("/api/v1/observer/vikingdb", format).await
    }

    /// Returns observer model status (`GET /api/v1/observer/models`).
    pub async fn models_status(&self, format: Option<ObserverFormat>) -> Result<JsonObject> {
        self.observer_get("/api/v1/observer/models", format).await
    }

    /// Returns aggregate observer system status (`GET /api/v1/observer/system`).
    pub async fn get_status(&self, format: Option<ObserverFormat>) -> Result<JsonObject> {
        self.observer_get("/api/v1/observer/system", format).await
    }

    /// Returns `true` when `GET /api/v1/observer/system` reports `is_healthy == true`.
    pub async fn is_healthy(&self) -> Result<bool> {
        let status = self.get_status(None).await?;
        Ok(status
            .get("is_healthy")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    async fn observer_get(&self, path: &str, format: Option<ObserverFormat>) -> Result<JsonObject> {
        let mut query: QueryPairs = Vec::new();
        if let Some(fmt) = format {
            query.push(("format".to_owned(), fmt.as_str().to_owned()));
        }
        self.transport
            .request_result(Method::GET, path, &query, None)
            .await
    }
}
