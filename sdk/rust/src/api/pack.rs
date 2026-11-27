// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! OVPack export, backup, import, and restore operations (`/api/v1/pack/*`).

use std::path::Path;

use http::Method;
use serde_json::{Map, Value};

use crate::client::AsyncHttpClient;
use crate::error::{ErrorKind, OpenVikingError, Result};
use crate::options::ImportPackOptions;
use crate::transport::Transport;
use crate::types::ResponseEnvelope;
use crate::uploads::{atomic_write_bytes, resolve_ovpack_output_path};
use crate::uri::VikingUri;

impl AsyncHttpClient {
    /// Exports a subtree URI into a local `.ovpack` file (`POST /api/v1/pack/export`) and returns its path.
    pub async fn export_ovpack(
        &self,
        uri: &str,
        to: &str,
        include_vectors: bool,
    ) -> Result<String> {
        let normalized_uri = VikingUri::normalize(uri);
        let out_path = resolve_ovpack_output_path(to, Some(&normalized_uri), "export");

        let mut payload = Map::new();
        payload.insert("uri".to_owned(), Value::String(normalized_uri));
        payload.insert("include_vectors".to_owned(), Value::Bool(include_vectors));

        self.download_ovpack("/api/v1/pack/export", &Value::Object(payload), &out_path)
            .await?;
        Ok(out_path.to_string_lossy().into_owned())
    }

    /// Backs up public scopes into a local `.ovpack` file (`POST /api/v1/pack/backup`) and returns its path.
    pub async fn backup_ovpack(&self, to: &str, include_vectors: bool) -> Result<String> {
        let out_path = resolve_ovpack_output_path(to, None, "openviking-backup");

        let mut payload = Map::new();
        payload.insert("include_vectors".to_owned(), Value::Bool(include_vectors));

        self.download_ovpack("/api/v1/pack/backup", &Value::Object(payload), &out_path)
            .await?;
        Ok(out_path.to_string_lossy().into_owned())
    }

    /// Imports a local `.ovpack` archive under `parent` (`POST /api/v1/pack/import`).
    pub async fn import_ovpack(
        &self,
        file_path: &str,
        parent: &str,
        options: ImportPackOptions,
    ) -> Result<String> {
        let path = validate_local_pack_file(file_path).await?;
        let temp_file_id = self.transport.upload_temp_file(path).await?;

        let mut payload = Map::new();
        payload.insert(
            "parent".to_owned(),
            Value::String(VikingUri::normalize(parent)),
        );
        payload.insert("temp_file_id".to_owned(), Value::String(temp_file_id));
        if let Some(on_conflict) = options.on_conflict {
            payload.insert("on_conflict".to_owned(), Value::String(on_conflict));
        }
        if let Some(vector_mode) = options.vector_mode {
            payload.insert("vector_mode".to_owned(), Value::String(vector_mode));
        }

        let result: crate::error::JsonObject = self
            .transport
            .request_result(
                Method::POST,
                "/api/v1/pack/import",
                &[],
                Some(&Value::Object(payload)),
            )
            .await?;
        Ok(result
            .get("uri")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned())
    }

    /// Restores a local backup `.ovpack` archive (`POST /api/v1/pack/restore`).
    pub async fn restore_ovpack(
        &self,
        file_path: &str,
        options: ImportPackOptions,
    ) -> Result<String> {
        let path = validate_local_pack_file(file_path).await?;
        let temp_file_id = self.transport.upload_temp_file(path).await?;

        let mut payload = Map::new();
        payload.insert("temp_file_id".to_owned(), Value::String(temp_file_id));
        if let Some(on_conflict) = options.on_conflict {
            payload.insert("on_conflict".to_owned(), Value::String(on_conflict));
        }
        if let Some(vector_mode) = options.vector_mode {
            payload.insert("vector_mode".to_owned(), Value::String(vector_mode));
        }

        let result: crate::error::JsonObject = self
            .transport
            .request_result(
                Method::POST,
                "/api/v1/pack/restore",
                &[],
                Some(&Value::Object(payload)),
            )
            .await?;
        Ok(result
            .get("uri")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned())
    }

    async fn download_ovpack(&self, route: &str, body: &Value, dest: &Path) -> Result<()> {
        let response = self
            .transport
            .send_raw(Method::POST, route, &[], Some(body))
            .await?;
        if !response.status().is_success() {
            let _: ResponseEnvelope<Value> = Transport::parse_envelope(response).await?;
            return Ok(());
        }
        let bytes = response.bytes().await?;
        atomic_write_bytes(dest, &bytes).await
    }
}

async fn validate_local_pack_file(file_path: &str) -> Result<&Path> {
    let path = Path::new(file_path);
    let meta = match tokio::fs::metadata(path).await {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(OpenVikingError::new(
                ErrorKind::Io,
                "FILE_NOT_FOUND",
                format!("Local ovpack file not found: {file_path}"),
            )
            .with_source(err));
        }
        Err(err) => return Err(err.into()),
    };
    if !meta.is_file() {
        return Err(OpenVikingError::invalid_argument(format!(
            "Path {file_path} is not a file"
        )));
    }
    Ok(path)
}
