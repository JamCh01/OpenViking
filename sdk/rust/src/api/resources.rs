// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Resource import endpoint (`POST /api/v1/resources`).

use http::Method;
use serde_json::{Map, Value};

use crate::client::AsyncHttpClient;
use crate::error::{JsonObject, OpenVikingError, Result};
use crate::options::{compact_request_body, merge_extra, AddResourceOptions};
use crate::uploads::{expand_user_path, zip_directory_in};
use crate::uri::VikingUri;

impl AsyncHttpClient {
    /// Imports a local file, local directory, remote URL, or external connector source into OpenViking.
    ///
    /// Local files and directories are automatically uploaded to `/api/v1/resources/temp_upload`
    /// unless `options.add_type` specifies a server-side connector.
    pub async fn add_resource(
        &self,
        path: &str,
        options: AddResourceOptions,
    ) -> Result<JsonObject> {
        let add_type = options
            .add_type
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);

        let to_non_empty = options.to.as_deref().filter(|s| !s.is_empty());
        let parent_non_empty = options.parent.as_deref().filter(|s| !s.is_empty());

        if add_type.is_some() && parent_non_empty.is_some() {
            return Err(OpenVikingError::invalid_argument(
                "'add_type' cannot be combined with 'parent'.",
            ));
        }
        if add_type.is_some() && to_non_empty.is_none() {
            return Err(OpenVikingError::invalid_argument(
                "'add_type' requires an exact 'to' target.",
            ));
        }
        if to_non_empty.is_some() && parent_non_empty.is_some() {
            return Err(OpenVikingError::invalid_argument(
                "Cannot specify both 'to' and 'parent' at the same time.",
            ));
        }

        let mut payload = Map::new();
        payload.insert("wait".to_owned(), Value::Bool(options.wait));
        if let Some(to) = to_non_empty {
            payload.insert("to".to_owned(), Value::String(VikingUri::normalize(to)));
        }
        if let Some(parent) = parent_non_empty {
            payload.insert(
                "parent".to_owned(),
                Value::String(VikingUri::normalize(parent)),
            );
        }
        if let Some(kind) = &add_type {
            payload.insert("add_type".to_owned(), Value::String(kind.clone()));
        }
        if let Some(acl) = &options.acl {
            payload.insert("acl".to_owned(), serde_json::to_value(acl)?);
        }
        if let Some(reason) = &options.reason {
            payload.insert("reason".to_owned(), Value::String(reason.clone()));
        }
        if let Some(instruction) = &options.instruction {
            payload.insert("instruction".to_owned(), Value::String(instruction.clone()));
        }
        if let Some(create_parent) = options.create_parent {
            payload.insert("create_parent".to_owned(), Value::Bool(create_parent));
        }
        if let Some(strict) = options.strict {
            payload.insert("strict".to_owned(), Value::Bool(strict));
        }
        if let Some(ignore_dirs) = &options.ignore_dirs {
            payload.insert("ignore_dirs".to_owned(), Value::String(ignore_dirs.clone()));
        }
        if let Some(include) = &options.include {
            payload.insert("include".to_owned(), Value::String(include.clone()));
        }
        if let Some(exclude) = &options.exclude {
            payload.insert("exclude".to_owned(), Value::String(exclude.clone()));
        }
        if let Some(direct_media) = options.directly_upload_media {
            payload.insert(
                "directly_upload_media".to_owned(),
                Value::Bool(direct_media),
            );
        }
        if let Some(preserve_structure) = options.preserve_structure {
            payload.insert(
                "preserve_structure".to_owned(),
                Value::Bool(preserve_structure),
            );
        }
        if let Some(watch_interval) = options.watch_interval {
            if let Some(num) = serde_json::Number::from_f64(watch_interval) {
                payload.insert("watch_interval".to_owned(), Value::Number(num));
            }
        }
        if let Some(args) = &options.args {
            if !args.is_empty() {
                payload.insert("args".to_owned(), Value::Object(args.clone()));
            }
        }
        if let Some(telemetry) = &options.telemetry {
            payload.insert("telemetry".to_owned(), telemetry.clone());
        }
        if let Some(processing_mode) = options.processing_mode {
            payload.insert(
                "processing_mode".to_owned(),
                Value::String(processing_mode.as_str().to_owned()),
            );
        }
        if let Some(tags) = &options.tags {
            payload.insert(
                "tags".to_owned(),
                Value::Array(tags.iter().cloned().map(Value::String).collect()),
            );
        }
        if let Some(tag_mode) = options.tag_mode {
            payload.insert(
                "tag_mode".to_owned(),
                Value::String(tag_mode.as_str().to_owned()),
            );
        }

        merge_extra(
            &mut payload,
            &options.extra,
            &[
                "to",
                "parent",
                "wait",
                "path",
                "temp_file_id",
                "source_name",
            ],
        )?;

        let candidate = expand_user_path(path);
        if add_type.is_none() {
            if let Ok(meta) = tokio::fs::metadata(&candidate).await {
                if meta.is_dir() {
                    if let Some(name) = candidate
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                    {
                        payload.insert("source_name".to_owned(), Value::String(name));
                    }
                    let zip_path = zip_directory_in(&candidate, self.temp_dir_path()).await?;
                    let upload_res = self.transport.upload_temp_file(&zip_path).await;
                    let _ = tokio::fs::remove_file(&zip_path).await;
                    let temp_file_id = upload_res?;
                    payload.insert("temp_file_id".to_owned(), Value::String(temp_file_id));
                } else if meta.is_file() {
                    if let Some(name) = candidate
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                    {
                        payload.insert("source_name".to_owned(), Value::String(name));
                    }
                    let temp_file_id = self.transport.upload_temp_file(&candidate).await?;
                    payload.insert("temp_file_id".to_owned(), Value::String(temp_file_id));
                } else {
                    payload.insert("path".to_owned(), Value::String(path.to_owned()));
                }
            } else {
                payload.insert("path".to_owned(), Value::String(path.to_owned()));
            }
        } else {
            payload.insert("path".to_owned(), Value::String(path.to_owned()));
        }

        compact_request_body(&mut payload);
        self.transport
            .request_result(
                Method::POST,
                "/api/v1/resources",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }
}
