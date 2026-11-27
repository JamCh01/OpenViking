// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Agent skill management endpoints (`/api/v1/skills*`).

use http::Method;
use serde_json::{Map, Value};

use crate::client::AsyncHttpClient;
use crate::error::{JsonObject, Result};
use crate::options::{
    compact_request_body, merge_extra, AddSkillOptions, FindSkillsOptions, GetSkillOptions,
    SkillSource, UpdateSkillOptions, ValidateSkillOptions,
};
use crate::transport::{encode_path_segment, QueryPairs};
use crate::uploads::{expand_user_path, is_inline_content_path_error, zip_directory_in};
use crate::uri::VikingUri;

impl AsyncHttpClient {
    /// Installs a skill from a local file, local directory, inline Markdown string, or JSON object.
    pub async fn add_skill(
        &self,
        data: impl Into<SkillSource>,
        options: AddSkillOptions,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("wait".to_owned(), Value::Bool(options.wait));
        if let Some(telemetry) = &options.telemetry {
            payload.insert("telemetry".to_owned(), telemetry.clone());
        }
        if let Some(target_uri) = &options.target_uri {
            payload.insert(
                "target_uri".to_owned(),
                Value::String(VikingUri::normalize(target_uri)),
            );
        }
        merge_extra(
            &mut payload,
            &options.extra,
            &["wait", "data", "temp_file_id"],
        )?;
        compact_request_body(&mut payload);
        self.attach_skill_payload(&mut payload, data.into()).await?;

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/skills",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Lists installed skills (`GET /api/v1/skills`).
    pub async fn list_skills(
        &self,
        node_limit: usize,
        target_uri: Option<&str>,
    ) -> Result<JsonObject> {
        let mut query: QueryPairs = vec![("node_limit".to_owned(), node_limit.to_string())];
        if let Some(uri) = target_uri {
            query.push(("target_uri".to_owned(), uri.to_owned()));
        }
        self.transport
            .request_result(Method::GET, "/api/v1/skills", &query, None)
            .await
    }

    /// Searches installed skills semantically (`POST /api/v1/skills/find`).
    pub async fn find_skills(&self, query: &str, options: FindSkillsOptions) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("query".to_owned(), Value::String(query.to_owned()));
        payload.insert("limit".to_owned(), Value::Number(options.limit.into()));
        payload.insert(
            "score_threshold".to_owned(),
            match options
                .score_threshold
                .and_then(serde_json::Number::from_f64)
            {
                Some(num) => Value::Number(num),
                None => Value::Null,
            },
        );
        payload.insert(
            "level".to_owned(),
            match options.level {
                Some(levels) => Value::Array(
                    levels
                        .into_iter()
                        .map(|n| Value::Number(n.into()))
                        .collect(),
                ),
                None => Value::Null,
            },
        );
        payload.insert("telemetry".to_owned(), options.telemetry);
        if let Some(target_uri) = options.target_uri {
            payload.insert("target_uri".to_owned(), Value::String(target_uri));
        }

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/skills/find",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Validates a skill definition without installing it (`POST /api/v1/skills/validate`).
    pub async fn validate_skill(
        &self,
        data: impl Into<SkillSource>,
        options: ValidateSkillOptions,
    ) -> Result<JsonObject> {
        let wire_data = match data.into() {
            SkillSource::TextOrPath(text) => Value::String(text),
            SkillSource::Json(val) => val,
        };
        let mut payload = Map::new();
        payload.insert("data".to_owned(), wire_data);
        payload.insert("strict".to_owned(), Value::Bool(options.strict));
        if let Some(source_path) = options.source_path {
            payload.insert("source_path".to_owned(), Value::String(source_path));
        }
        if let Some(skill_dir_name) = options.skill_dir_name {
            payload.insert("skill_dir_name".to_owned(), Value::String(skill_dir_name));
        }
        if let Some(target_uri) = options.target_uri {
            payload.insert("target_uri".to_owned(), Value::String(target_uri));
        }

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/skills/validate",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Retrieves metadata and optional content for one installed skill (`GET /api/v1/skills/{name}`).
    pub async fn get_skill(
        &self,
        skill_name: &str,
        options: GetSkillOptions,
    ) -> Result<JsonObject> {
        let mut query: QueryPairs = vec![
            (
                "include_files".to_owned(),
                options.include_files.to_string(),
            ),
            (
                "include_integrity".to_owned(),
                options.include_integrity.to_string(),
            ),
            (
                "include_source".to_owned(),
                options.include_source.to_string(),
            ),
        ];
        if let Some(include_content) = options.include_content {
            query.push(("include_content".to_owned(), include_content.to_string()));
        }
        if let Some(level) = options.level {
            query.push(("level".to_owned(), level.to_string()));
        }
        if let Some(target_uri) = options.target_uri {
            query.push(("target_uri".to_owned(), target_uri));
        }
        let encoded = encode_path_segment(skill_name);
        self.transport
            .request_result(
                Method::GET,
                &format!("/api/v1/skills/{encoded}"),
                &query,
                None,
            )
            .await
    }

    /// Replaces an installed skill (`PUT /api/v1/skills/{name}`).
    pub async fn update_skill(
        &self,
        skill_name: &str,
        data: impl Into<SkillSource>,
        options: UpdateSkillOptions,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("wait".to_owned(), Value::Bool(options.wait));
        if let Some(telemetry) = &options.telemetry {
            payload.insert("telemetry".to_owned(), telemetry.clone());
        }
        if let Some(target_uri) = &options.target_uri {
            payload.insert(
                "target_uri".to_owned(),
                Value::String(VikingUri::normalize(target_uri)),
            );
        }
        if let Some(source_metadata) = &options.source_metadata {
            payload.insert(
                "source_metadata".to_owned(),
                Value::Object(source_metadata.clone()),
            );
        }
        merge_extra(
            &mut payload,
            &options.extra,
            &["wait", "data", "temp_file_id"],
        )?;
        compact_request_body(&mut payload);
        self.attach_skill_payload(&mut payload, data.into()).await?;

        let encoded = encode_path_segment(skill_name);
        self.transport
            .request_result(
                Method::PUT,
                &format!("/api/v1/skills/{encoded}"),
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Deletes an installed skill (`DELETE /api/v1/skills/{name}`).
    pub async fn delete_skill(
        &self,
        skill_name: &str,
        target_uri: Option<&str>,
    ) -> Result<JsonObject> {
        let mut query: QueryPairs = Vec::new();
        if let Some(uri) = target_uri {
            query.push(("target_uri".to_owned(), uri.to_owned()));
        }
        let encoded = encode_path_segment(skill_name);
        self.transport
            .request_result(
                Method::DELETE,
                &format!("/api/v1/skills/{encoded}"),
                &query,
                None,
            )
            .await
    }

    async fn attach_skill_payload(
        &self,
        payload: &mut JsonObject,
        source: SkillSource,
    ) -> Result<()> {
        match source {
            SkillSource::Json(val) => {
                payload.insert("data".to_owned(), val);
            }
            SkillSource::TextOrPath(text) => {
                let candidate = expand_user_path(&text);
                match tokio::fs::metadata(&candidate).await {
                    Ok(meta) if meta.is_dir() => {
                        let zip_path = zip_directory_in(&candidate, self.temp_dir_path()).await?;
                        let upload_res = self.transport.upload_temp_file(&zip_path).await;
                        let _ = tokio::fs::remove_file(&zip_path).await;
                        payload.insert("temp_file_id".to_owned(), Value::String(upload_res?));
                    }
                    Ok(meta) if meta.is_file() => {
                        let temp_id = self.transport.upload_temp_file(&candidate).await?;
                        payload.insert("temp_file_id".to_owned(), Value::String(temp_id));
                    }
                    Ok(_) => {
                        payload.insert("data".to_owned(), Value::String(text));
                    }
                    Err(io_err)
                        if io_err.kind() == std::io::ErrorKind::NotFound
                            || is_inline_content_path_error(&io_err) =>
                    {
                        payload.insert("data".to_owned(), Value::String(text));
                    }
                    Err(io_err) => {
                        return Err(io_err.into());
                    }
                }
            }
        }
        Ok(())
    }
}
