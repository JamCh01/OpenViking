// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Background tasks, compilation (`/api/v1/compile*`), and durable rules (`/api/v1/rules/*`).

use http::Method;
use serde_json::{Map, Value};

use crate::client::AsyncHttpClient;
use crate::error::{JsonObject, OpenVikingError, Result};
use crate::options::{
    compact_request_body, merge_extra, CompileOptions, CompileRequest, ListTasksOptions,
    RuleCompileOptions, RuleCompileSource, RuleSearchOptions,
};
use crate::transport::{encode_path_segment, QueryPairs};
use crate::uploads::{expand_user_path, zip_directory_in};
use crate::uri::VikingUri;

impl AsyncHttpClient {
    /// Retrieves a background task (`GET /api/v1/tasks/{task_id}`), returning `Ok(None)` on HTTP 404.
    pub async fn get_task(&self, task_id: &str) -> Result<Option<JsonObject>> {
        let encoded = encode_path_segment(task_id);
        match self
            .transport
            .request_result::<JsonObject>(
                Method::GET,
                &format!("/api/v1/tasks/{encoded}"),
                &[],
                None,
            )
            .await
        {
            Ok(task) => Ok(Some(task)),
            Err(err) if err.is_not_found() => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// Requests cooperative cancellation of a background task (`POST /api/v1/tasks/{task_id}/cancel`).
    pub async fn cancel_task(&self, task_id: &str) -> Result<JsonObject> {
        let encoded = encode_path_segment(task_id);
        self.transport
            .request_result(
                Method::POST,
                &format!("/api/v1/tasks/{encoded}/cancel"),
                &[],
                None,
            )
            .await
    }

    /// Lists background tasks visible to the caller (`GET /api/v1/tasks`).
    pub async fn list_tasks(&self, options: ListTasksOptions) -> Result<Vec<JsonObject>> {
        let mut query: QueryPairs = vec![("limit".to_owned(), options.limit.to_string())];
        if let Some(task_type) = options.task_type {
            query.push(("task_type".to_owned(), task_type));
        }
        if let Some(status) = options.status {
            query.push(("status".to_owned(), status));
        }
        if let Some(resource_id) = options.resource_id {
            query.push(("resource_id".to_owned(), resource_id));
        }
        self.transport
            .request_result(Method::GET, "/api/v1/tasks", &query, None)
            .await
    }

    /// Saves a persistent Compile configuration without starting a run (`POST /api/v1/compile/create`).
    pub async fn create_compile_task(
        &self,
        compile_name: &str,
        from_uris: &[String],
        to: &str,
        skill: &str,
        options: CompileOptions,
    ) -> Result<JsonObject> {
        if compile_name.is_empty() {
            return Err(OpenVikingError::invalid_argument(
                "compile_name is required",
            ));
        }

        let mut payload = Map::new();
        payload.insert(
            "compile_name".to_owned(),
            Value::String(compile_name.to_owned()),
        );
        payload.insert(
            "from".to_owned(),
            Value::Array(from_uris.iter().cloned().map(Value::String).collect()),
        );
        payload.insert("to".to_owned(), Value::String(to.to_owned()));
        payload.insert("skill".to_owned(), Value::String(skill.to_owned()));
        if let Some(instruction) = options.instruction {
            payload.insert("instruction".to_owned(), Value::String(instruction));
        }
        if let Some(args) = options.args {
            if !args.is_empty() {
                payload.insert("args".to_owned(), Value::Object(args));
            }
        }
        merge_extra(
            &mut payload,
            &options.extra,
            &["compile_name", "from", "to", "skill"],
        )?;
        compact_request_body(&mut payload);

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/compile/create",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Queues a saved persistent compile task or a full standalone compilation (`POST /api/v1/compile`).
    pub async fn compile(&self, request: CompileRequest) -> Result<JsonObject> {
        let mut payload = Map::new();
        match request {
            CompileRequest::Named {
                compile_name,
                options,
            } => {
                if compile_name.is_empty() {
                    return Err(OpenVikingError::invalid_argument(
                        "compile_name must not be empty",
                    ));
                }
                if options.instruction.is_some() || !options.extra.is_empty() {
                    return Err(OpenVikingError::invalid_argument(
                        "compile_name cannot be combined with Compile inputs",
                    ));
                }
                payload.insert("compile_name".to_owned(), Value::String(compile_name));
                if let Some(args) = options.args {
                    if !args.is_empty() {
                        payload.insert("args".to_owned(), Value::Object(args));
                    }
                }
            }
            CompileRequest::Full {
                from_uris,
                to,
                skill,
                options,
            } => {
                if skill.is_empty() || to.is_empty() || (skill != "memory" && from_uris.is_empty())
                {
                    return Err(OpenVikingError::invalid_argument(
                        "Full Compile requires to, skill and non-memory from_uris",
                    ));
                }
                payload.insert(
                    "from".to_owned(),
                    Value::Array(from_uris.into_iter().map(Value::String).collect()),
                );
                payload.insert("to".to_owned(), Value::String(to));
                payload.insert("skill".to_owned(), Value::String(skill));
                if let Some(instruction) = options.instruction {
                    payload.insert("instruction".to_owned(), Value::String(instruction));
                }
                if let Some(args) = options.args {
                    if !args.is_empty() {
                        payload.insert("args".to_owned(), Value::Object(args));
                    }
                }
                merge_extra(&mut payload, &options.extra, &["from", "to", "skill"])?;
            }
        }
        compact_request_body(&mut payload);

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/compile",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Compiles inline instructions and resource/local-file sources into durable rules
    /// (`POST /api/v1/rules/compile`).
    pub async fn rule_compile(
        &self,
        instruction: Option<&str>,
        options: RuleCompileOptions,
    ) -> Result<JsonObject> {
        let mut wire_sources = Vec::with_capacity(options.sources.len());
        for source in options.sources {
            match source {
                RuleCompileSource::Structured(mut map) => {
                    if let Some(res_uri) = map
                        .get("resource_uri")
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                        .map(VikingUri::normalize)
                    {
                        map.insert("resource_uri".to_owned(), Value::String(res_uri));
                        wire_sources.push(Value::Object(map));
                        continue;
                    }

                    for target_field in ["to", "parent"] {
                        if let Some(normalized) = map
                            .get(target_field)
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty())
                            .map(VikingUri::normalize)
                        {
                            map.insert(target_field.to_owned(), Value::String(normalized));
                        }
                    }

                    let has_add_type = map
                        .get("add_type")
                        .and_then(Value::as_str)
                        .is_some_and(|s| !s.is_empty());
                    if !has_add_type {
                        if let Some(path_str) =
                            map.get("path").and_then(Value::as_str).map(str::to_owned)
                        {
                            let local_path = expand_user_path(&path_str);
                            if let Ok(meta) = tokio::fs::metadata(&local_path).await {
                                if meta.is_dir() || meta.is_file() {
                                    map.remove("path");
                                    if !map.contains_key("source_name") {
                                        if let Some(name) = local_path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().into_owned())
                                        {
                                            map.insert(
                                                "source_name".to_owned(),
                                                Value::String(name),
                                            );
                                        }
                                    }
                                    let temp_id = if meta.is_dir() {
                                        let zip_path =
                                            zip_directory_in(&local_path, self.temp_dir_path())
                                                .await?;
                                        let res = self.transport.upload_temp_file(&zip_path).await;
                                        let _ = tokio::fs::remove_file(&zip_path).await;
                                        res?
                                    } else {
                                        self.transport.upload_temp_file(&local_path).await?
                                    };
                                    map.insert("temp_file_id".to_owned(), Value::String(temp_id));
                                }
                            }
                        }
                    }
                    wire_sources.push(Value::Object(map));
                }
                RuleCompileSource::PathOrUri(value) => {
                    if value.starts_with("viking://") {
                        let mut item = Map::new();
                        item.insert(
                            "resource_uri".to_owned(),
                            Value::String(VikingUri::normalize(&value)),
                        );
                        wire_sources.push(Value::Object(item));
                        continue;
                    }

                    let local_path = expand_user_path(&value);
                    match tokio::fs::metadata(&local_path).await {
                        Ok(meta) if meta.is_dir() => {
                            let zip_path =
                                zip_directory_in(&local_path, self.temp_dir_path()).await?;
                            let upload_res = self.transport.upload_temp_file(&zip_path).await;
                            let _ = tokio::fs::remove_file(&zip_path).await;
                            let mut item = Map::new();
                            item.insert("temp_file_id".to_owned(), Value::String(upload_res?));
                            if let Some(name) = local_path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                            {
                                item.insert("source_name".to_owned(), Value::String(name));
                            }
                            wire_sources.push(Value::Object(item));
                        }
                        Ok(meta) if meta.is_file() => {
                            let temp_id = self.transport.upload_temp_file(&local_path).await?;
                            let mut item = Map::new();
                            item.insert("temp_file_id".to_owned(), Value::String(temp_id));
                            if let Some(name) = local_path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                            {
                                item.insert("source_name".to_owned(), Value::String(name));
                            }
                            wire_sources.push(Value::Object(item));
                        }
                        _ => {
                            let mut item = Map::new();
                            item.insert("path".to_owned(), Value::String(value));
                            wire_sources.push(Value::Object(item));
                        }
                    }
                }
            }
        }

        let mut payload = Map::new();
        if let Some(instr) = instruction {
            payload.insert("instruction".to_owned(), Value::String(instr.to_owned()));
        }
        payload.insert("sources".to_owned(), Value::Array(wire_sources));
        compact_request_body(&mut payload);

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/rules/compile",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Searches active compiled rules applicable to `task_context` (`POST /api/v1/rules/search`).
    pub async fn rule_search(
        &self,
        task_context: &str,
        options: RuleSearchOptions,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert(
            "task_context".to_owned(),
            Value::String(task_context.to_owned()),
        );
        if let Some(threshold) = options
            .rule_probability_threshold
            .and_then(serde_json::Number::from_f64)
        {
            payload.insert(
                "rule_probability_threshold".to_owned(),
                Value::Number(threshold),
            );
        }
        merge_extra(&mut payload, &options.extra, &["task_context"])?;
        compact_request_body(&mut payload);

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/rules/search",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }
}
