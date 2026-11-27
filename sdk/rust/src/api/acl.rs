// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Access control list (ACL) endpoints (`/api/v1/acl*`).

use http::Method;
use serde_json::{Map, Value};

use crate::client::AsyncHttpClient;
use crate::error::{JsonObject, OpenVikingError, Result};
use crate::transport::QueryPairs;
use crate::types::AclEntry;
use crate::uri::VikingUri;

impl AsyncHttpClient {
    /// Returns direct, inherited, and effective ACL state for a `viking://` URI (`GET /api/v1/acl`).
    pub async fn acl_get(&self, uri: &str) -> Result<JsonObject> {
        let query: QueryPairs = vec![("uri".to_owned(), VikingUri::normalize(uri))];
        self.transport
            .request_result(Method::GET, "/api/v1/acl", &query, None)
            .await
    }

    /// Updates direct ACL entries and/or `acl_mode` (`"inherit"` or `"restricted"`) on a URI (`PUT /api/v1/acl`).
    pub async fn acl_set(
        &self,
        uri: &str,
        entries: Option<&[AclEntry]>,
        acl_mode: Option<&str>,
    ) -> Result<JsonObject> {
        if entries.is_none() && acl_mode.is_none() {
            return Err(OpenVikingError::invalid_argument(
                "Either entries or acl_mode must be provided",
            ));
        }
        let mut payload = Map::new();
        payload.insert("uri".to_owned(), Value::String(VikingUri::normalize(uri)));
        if let Some(entry_list) = entries {
            payload.insert("entries".to_owned(), serde_json::to_value(entry_list)?);
        }
        if let Some(mode) = acl_mode {
            payload.insert("acl_mode".to_owned(), Value::String(mode.to_owned()));
        }

        self.transport
            .request_result(
                Method::PUT,
                "/api/v1/acl",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Grants a direct access level (`"read"`, `"write"`, or `"manage"`) to `principal` (`POST /api/v1/acl/grant`).
    pub async fn acl_grant(&self, uri: &str, principal: &str, level: &str) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("uri".to_owned(), Value::String(VikingUri::normalize(uri)));
        payload.insert("principal".to_owned(), Value::String(principal.to_owned()));
        payload.insert("level".to_owned(), Value::String(level.to_owned()));

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/acl/grant",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Revokes a direct ACL entry for `principal` (`POST /api/v1/acl/revoke`).
    pub async fn acl_revoke(&self, uri: &str, principal: &str) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("uri".to_owned(), Value::String(VikingUri::normalize(uri)));
        payload.insert("principal".to_owned(), Value::String(principal.to_owned()));

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/acl/revoke",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Clears direct ACL entries and resets restricted mode on a URI (`DELETE /api/v1/acl`).
    pub async fn acl_delete(&self, uri: &str) -> Result<JsonObject> {
        let query: QueryPairs = vec![("uri".to_owned(), VikingUri::normalize(uri))];
        self.transport
            .request_result(Method::DELETE, "/api/v1/acl", &query, None)
            .await
    }
}
