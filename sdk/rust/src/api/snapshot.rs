// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Snapshot / Git version-control endpoints (`/api/v1/snapshot/*`) and `SnapshotNamespace`.

use http::Method;
use serde_json::{Map, Value};

use crate::client::AsyncHttpClient;
use crate::error::{JsonObject, Result};
use crate::options::{
    SnapshotCommitOptions, SnapshotDiffPathsOptions, SnapshotLogOptions, SnapshotRestoreOptions,
};
use crate::transport::{QueryPairs, Transport};
use crate::types::{ResponseEnvelope, SnapshotBlob, SnapshotShowResult};

/// Async snapshot version-control sub-namespace returned by [`AsyncHttpClient::snapshot`].
#[derive(Debug, Clone)]
pub struct SnapshotNamespace {
    client: AsyncHttpClient,
}

impl SnapshotNamespace {
    pub(crate) fn new(client: AsyncHttpClient) -> Self {
        Self { client }
    }

    /// Creates a snapshot commit (`POST /api/v1/snapshot/commit`).
    pub async fn commit(&self, options: SnapshotCommitOptions) -> Result<JsonObject> {
        self.client.git_commit(options).await
    }

    /// Restores workspace state from a previous commit (`POST /api/v1/snapshot/restore`).
    pub async fn restore(&self, options: SnapshotRestoreOptions) -> Result<JsonObject> {
        self.client.git_restore(options).await
    }

    /// Inspects commit metadata (`path = None`) or fetches a raw file blob (`path = Some(uri)`).
    pub async fn show(&self, target_ref: &str, path: Option<&str>) -> Result<SnapshotShowResult> {
        self.client.git_show(target_ref, path).await
    }

    /// Lists commit history newest-first (`GET /api/v1/snapshot/log`).
    pub async fn log(&self, options: SnapshotLogOptions) -> Result<Vec<JsonObject>> {
        self.client.git_log(options).await
    }

    /// Compares a single file between two snapshot refs (`GET /api/v1/snapshot/diff`).
    pub async fn diff(
        &self,
        path: &str,
        to_ref: &str,
        from_ref: Option<&str>,
    ) -> Result<JsonObject> {
        self.client.git_diff(path, to_ref, from_ref).await
    }

    /// Pages changed source URIs under directories (`GET /api/v1/snapshot/diff-paths`).
    pub async fn diff_paths(
        &self,
        paths: &[String],
        options: SnapshotDiffPathsOptions,
    ) -> Result<JsonObject> {
        self.client.git_diff_paths(paths, options).await
    }

    /// Returns the account `.ovgitignore` content (`GET /api/v1/snapshot/ignore`).
    pub async fn get_gitignore(&self) -> Result<String> {
        self.client.git_get_ignore().await
    }

    /// Writes the account `.ovgitignore` control file (`PUT /api/v1/snapshot/ignore`).
    pub async fn set_gitignore(&self, content: &str) -> Result<()> {
        self.client.git_set_ignore(content).await
    }

    /// Deletes the account `.ovgitignore` control file (`DELETE /api/v1/snapshot/ignore`).
    pub async fn delete_gitignore(&self) -> Result<()> {
        self.client.git_delete_ignore().await
    }
}

impl AsyncHttpClient {
    /// Creates a snapshot commit (`POST /api/v1/snapshot/commit`).
    pub async fn git_commit(&self, options: SnapshotCommitOptions) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("message".to_owned(), Value::String(options.message));
        payload.insert("branch".to_owned(), Value::String(options.branch));
        if let Some(paths) = options.paths {
            payload.insert(
                "paths".to_owned(),
                Value::Array(paths.into_iter().map(Value::String).collect()),
            );
        }
        if let Some(author_name) = options.author_name {
            payload.insert("author_name".to_owned(), Value::String(author_name));
        }
        if let Some(author_email) = options.author_email {
            payload.insert("author_email".to_owned(), Value::String(author_email));
        }

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/snapshot/commit",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Restores a subtree or full account tree from `source_commit` (`POST /api/v1/snapshot/restore`).
    pub async fn git_restore(&self, options: SnapshotRestoreOptions) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert(
            "source_commit".to_owned(),
            Value::String(options.source_commit),
        );
        payload.insert("branch".to_owned(), Value::String(options.branch));
        payload.insert("dry_run".to_owned(), Value::Bool(options.dry_run));
        if let Some(project_dir) = options.project_dir {
            payload.insert("project_dir".to_owned(), Value::String(project_dir));
        }
        if let Some(message) = options.message {
            payload.insert("message".to_owned(), Value::String(message));
        }
        if let Some(author_name) = options.author_name {
            payload.insert("author_name".to_owned(), Value::String(author_name));
        }
        if let Some(author_email) = options.author_email {
            payload.insert("author_email".to_owned(), Value::String(author_email));
        }

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/snapshot/restore",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Fetches commit metadata (`path = None`) or a binary blob (`path = Some(...)`) (`GET /api/v1/snapshot/show`).
    pub async fn git_show(
        &self,
        target_ref: &str,
        path: Option<&str>,
    ) -> Result<SnapshotShowResult> {
        let mut query: QueryPairs = vec![("target_ref".to_owned(), target_ref.to_owned())];
        if let Some(file_path) = path {
            query.push(("path".to_owned(), file_path.to_owned()));
        }

        if path.is_none() {
            let commit = self
                .transport
                .request_result::<JsonObject>(Method::GET, "/api/v1/snapshot/show", &query, None)
                .await?;
            return Ok(SnapshotShowResult::Commit(commit));
        }

        let response = self
            .transport
            .send_raw(Method::GET, "/api/v1/snapshot/show", &query, None)
            .await?;
        let is_octet_stream = response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.starts_with("application/octet-stream"));

        if response.status().is_success() && is_octet_stream {
            let oid = response
                .headers()
                .get("x-snapshot-oid")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_owned();
            let header_size = response
                .headers()
                .get("x-snapshot-size")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());
            let bytes = response.bytes().await?.to_vec();
            let size = header_size.unwrap_or(bytes.len() as u64);
            return Ok(SnapshotShowResult::Blob(SnapshotBlob { oid, size, bytes }));
        }

        let envelope: ResponseEnvelope<JsonObject> = Transport::parse_envelope(response).await?;
        Ok(SnapshotShowResult::Commit(
            envelope.result.unwrap_or_default(),
        ))
    }

    /// Walks commit history newest-first (`GET /api/v1/snapshot/log`).
    pub async fn git_log(&self, options: SnapshotLogOptions) -> Result<Vec<JsonObject>> {
        let mut query: QueryPairs = vec![
            ("branch".to_owned(), options.branch),
            ("limit".to_owned(), options.limit.to_string()),
            ("offset".to_owned(), options.offset.to_string()),
        ];
        if let Some(paths) = options.paths {
            for path in paths {
                query.push(("paths".to_owned(), path));
            }
        }
        self.transport
            .request_result(Method::GET, "/api/v1/snapshot/log", &query, None)
            .await
    }

    /// Compares one file between two snapshot refs (`GET /api/v1/snapshot/diff`).
    pub async fn git_diff(
        &self,
        path: &str,
        to_ref: &str,
        from_ref: Option<&str>,
    ) -> Result<JsonObject> {
        let mut query: QueryPairs = vec![
            ("path".to_owned(), path.to_owned()),
            ("to".to_owned(), to_ref.to_owned()),
        ];
        if let Some(from) = from_ref {
            query.push(("from".to_owned(), from.to_owned()));
        }
        self.transport
            .request_result(Method::GET, "/api/v1/snapshot/diff", &query, None)
            .await
    }

    /// Pages changed source URIs under directories (`GET /api/v1/snapshot/diff-paths`).
    pub async fn git_diff_paths(
        &self,
        paths: &[String],
        options: SnapshotDiffPathsOptions,
    ) -> Result<JsonObject> {
        let mut query: QueryPairs = Vec::with_capacity(paths.len() + 4);
        for path in paths {
            query.push(("paths".to_owned(), path.clone()));
        }
        query.push(("to".to_owned(), options.to_ref));
        query.push(("offset".to_owned(), options.offset.to_string()));
        query.push(("limit".to_owned(), options.limit.to_string()));
        if let Some(from) = options.from_ref {
            query.push(("from".to_owned(), from));
        }

        self.transport
            .request_result(Method::GET, "/api/v1/snapshot/diff-paths", &query, None)
            .await
    }

    /// Returns the account `.ovgitignore` content (`GET /api/v1/snapshot/ignore`).
    pub async fn git_get_ignore(&self) -> Result<String> {
        let result: Value = self
            .transport
            .request_result(Method::GET, "/api/v1/snapshot/ignore", &[], None)
            .await?;
        Ok(result.as_str().unwrap_or("").to_owned())
    }

    /// Writes the account `.ovgitignore` control file (`PUT /api/v1/snapshot/ignore`).
    pub async fn git_set_ignore(&self, content: &str) -> Result<()> {
        let mut payload = Map::new();
        payload.insert("content".to_owned(), Value::String(content.to_owned()));
        self.transport
            .request_unit(
                Method::PUT,
                "/api/v1/snapshot/ignore",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Deletes the account `.ovgitignore` control file (`DELETE /api/v1/snapshot/ignore`).
    pub async fn git_delete_ignore(&self) -> Result<()> {
        self.transport
            .request_unit(Method::DELETE, "/api/v1/snapshot/ignore", &[], None)
            .await
    }
}
