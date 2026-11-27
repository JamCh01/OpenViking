// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Semantic retrieval, context assembly, grep, and glob endpoints (`/api/v1/search/*`).

use http::Method;
use serde_json::{Map, Value};

use crate::client::AsyncHttpClient;
use crate::error::{JsonObject, Result};
use crate::options::{
    compact_request_body, merge_extra, FindOptions, GlobOptions, GrepOptions, SearchContextOptions,
    SearchOptions,
};
use crate::types::{SearchContextResult, TargetUri};
use crate::uploads::normalize_image_input;
use crate::uri::VikingUri;

impl AsyncHttpClient {
    /// Runs stateless semantic retrieval (`POST /api/v1/search/find`).
    pub async fn find(&self, query: &str, options: FindOptions) -> Result<JsonObject> {
        let mut payload = build_find_payload(query, &options).await?;
        merge_extra(
            &mut payload,
            &options.extra,
            &["query", "target_uri", "limit", "image_url"],
        )?;
        compact_request_body(&mut payload);

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/search/find",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Runs session-aware semantic or keyword retrieval (`POST /api/v1/search/search`).
    pub async fn search(&self, query: &str, options: SearchOptions) -> Result<JsonObject> {
        let mut payload = build_find_payload(query, &options.find).await?;
        if let Some(session_id) = options.session_id {
            payload.insert("session_id".to_owned(), Value::String(session_id));
        }
        if let Some(search_type) = options.search_type {
            payload.insert("search_type".to_owned(), Value::String(search_type));
        }
        if let Some(args) = options.args {
            if !args.is_empty() {
                payload.insert("args".to_owned(), Value::Object(args));
            }
        }
        merge_extra(
            &mut payload,
            &options.find.extra,
            &["query", "session_id", "target_uri", "limit", "image_url"],
        )?;
        compact_request_body(&mut payload);

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/search/search",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Assembles a token-budgeted context block on the server (`POST /api/v1/search/search`, `mode="context"`).
    pub async fn search_context(
        &self,
        query: &str,
        options: SearchContextOptions,
    ) -> Result<SearchContextResult> {
        let mut payload = Map::new();
        payload.insert("query".to_owned(), Value::String(query.to_owned()));
        payload.insert("mode".to_owned(), Value::String("context".to_owned()));
        payload.insert(
            "target_uri".to_owned(),
            options.target_uri.unwrap_or_default().to_normalized_value(),
        );
        payload.insert(
            "limit".to_owned(),
            Value::Number(options.limit.unwrap_or(10).into()),
        );

        if let Some(session_id) = options.session_id {
            payload.insert("session_id".to_owned(), Value::String(session_id));
        }
        if let Some(search_type) = options.search_type {
            payload.insert("search_type".to_owned(), Value::String(search_type));
        }
        if let Some(image) = &options.image {
            payload.insert("image_url".to_owned(), normalize_image_input(image).await?);
        }
        if let Some(node_limit) = options.node_limit {
            payload.insert("node_limit".to_owned(), Value::Number(node_limit.into()));
        }
        if let Some(threshold) = options
            .score_threshold
            .and_then(serde_json::Number::from_f64)
        {
            payload.insert("score_threshold".to_owned(), Value::Number(threshold));
        }
        if let Some(filter) = options.filter {
            payload.insert("filter".to_owned(), Value::Object(filter));
        }
        if let Some(context_type) = options.context_type {
            payload.insert("context_type".to_owned(), context_type);
        }
        if let Some(include_provenance) = options.include_provenance {
            payload.insert(
                "include_provenance".to_owned(),
                Value::Bool(include_provenance),
            );
        }
        if let Some(tags) = options.tags {
            payload.insert(
                "tags".to_owned(),
                Value::Array(tags.into_iter().map(Value::String).collect()),
            );
        }
        if let Some(since) = options.since {
            payload.insert("since".to_owned(), Value::String(since));
        }
        if let Some(until) = options.until {
            payload.insert("until".to_owned(), Value::String(until));
        }
        if let Some(time_field) = options.time_field {
            payload.insert("time_field".to_owned(), Value::String(time_field));
        }
        if let Some(decay) = options.events_time_decay_protection {
            payload.insert(
                "events_time_decay_protection".to_owned(),
                Value::String(decay),
            );
        }
        if let Some(expansion) = options.query_expansion {
            payload.insert("query_expansion".to_owned(), Value::String(expansion));
        }
        if let Some(max_tokens) = options.max_tokens {
            payload.insert("max_tokens".to_owned(), Value::Number(max_tokens.into()));
        }
        if let Some(quotas) = options.quotas {
            let quota_map: JsonObject = quotas
                .into_iter()
                .map(|(k, v)| (k, Value::Number(v.into())))
                .collect();
            payload.insert("quotas".to_owned(), Value::Object(quota_map));
        }
        if let Some(purpose) = options.purpose {
            payload.insert("purpose".to_owned(), Value::String(purpose));
        }
        if let Some(detail) = options.detail {
            payload.insert("detail".to_owned(), detail);
        }
        if let Some(dedup_turns) = options.dedup_turns {
            payload.insert("dedup_turns".to_owned(), Value::Number(dedup_turns.into()));
        }
        if let Some(exclude_uris) = options.exclude_uris {
            payload.insert(
                "exclude_uris".to_owned(),
                Value::Array(exclude_uris.into_iter().map(Value::String).collect()),
            );
        }
        if let Some(peer_scope) = options.peer_scope {
            payload.insert("peer_scope".to_owned(), Value::String(peer_scope));
        }
        if let Some(penalty) = options.other_peer_penalty {
            payload.insert("other_peer_penalty".to_owned(), penalty);
        }
        if let Some(rewrite) = options.rewrite {
            payload.insert("rewrite".to_owned(), rewrite);
        }
        if let Some(max_bullets) = options.rewrite_max_bullets {
            payload.insert(
                "rewrite_max_bullets".to_owned(),
                Value::Number(max_bullets.into()),
            );
        }
        if let Some(telemetry) = options.telemetry {
            payload.insert("telemetry".to_owned(), telemetry);
        }

        merge_extra(
            &mut payload,
            &options.extra,
            &[
                "query",
                "mode",
                "session_id",
                "target_uri",
                "limit",
                "image_url",
            ],
        )?;
        compact_request_body(&mut payload);

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/search/search",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Searches stored content by pattern across one or multiple root URIs (`POST /api/v1/search/grep`).
    pub async fn grep(
        &self,
        uri: impl Into<TargetUri>,
        pattern: &str,
        options: GrepOptions,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("uri".to_owned(), uri.into().to_normalized_value());
        payload.insert("pattern".to_owned(), Value::String(pattern.to_owned()));
        payload.insert(
            "case_insensitive".to_owned(),
            Value::Bool(options.case_insensitive),
        );
        payload.insert(
            "node_limit".to_owned(),
            Value::Number(options.node_limit.into()),
        );
        if let Some(level_limit) = options.level_limit {
            payload.insert("level_limit".to_owned(), Value::Number(level_limit.into()));
        }
        if let Some(exclude_uri) = options.exclude_uri {
            payload.insert(
                "exclude_uri".to_owned(),
                Value::String(VikingUri::normalize(&exclude_uri)),
            );
        }
        if let Some(tags) = options.tags {
            payload.insert(
                "tags".to_owned(),
                Value::Array(tags.into_iter().map(Value::String).collect()),
            );
        }
        if options.include_tags {
            payload.insert("include_tags".to_owned(), Value::Bool(true));
        }

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/search/grep",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Matches file paths by glob pattern across one or multiple root URIs (`POST /api/v1/search/glob`).
    pub async fn glob(&self, pattern: &str, options: GlobOptions) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("pattern".to_owned(), Value::String(pattern.to_owned()));
        payload.insert("uri".to_owned(), options.uri.to_normalized_value());
        payload.insert(
            "node_limit".to_owned(),
            Value::Number(options.node_limit.into()),
        );
        if let Some(extra_fields) = options.extra_fields {
            payload.insert(
                "extra_fields".to_owned(),
                Value::Array(extra_fields.into_iter().map(Value::String).collect()),
            );
        }
        if let Some(tags) = options.tags {
            payload.insert(
                "tags".to_owned(),
                Value::Array(tags.into_iter().map(Value::String).collect()),
            );
        }
        if options.include_tags {
            payload.insert("include_tags".to_owned(), Value::Bool(true));
        }

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/search/glob",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }
}

async fn build_find_payload(query: &str, options: &FindOptions) -> Result<JsonObject> {
    let mut payload = Map::new();
    payload.insert("query".to_owned(), Value::String(query.to_owned()));
    payload.insert(
        "target_uri".to_owned(),
        options
            .target_uri
            .clone()
            .unwrap_or_default()
            .to_normalized_value(),
    );
    payload.insert(
        "limit".to_owned(),
        Value::Number(options.limit.unwrap_or(10).into()),
    );

    if let Some(image) = &options.image {
        payload.insert("image_url".to_owned(), normalize_image_input(image).await?);
    }
    if let Some(node_limit) = options.node_limit {
        payload.insert("node_limit".to_owned(), Value::Number(node_limit.into()));
    }
    if let Some(threshold) = options
        .score_threshold
        .and_then(serde_json::Number::from_f64)
    {
        payload.insert("score_threshold".to_owned(), Value::Number(threshold));
    }
    if let Some(filter) = &options.filter {
        payload.insert("filter".to_owned(), Value::Object(filter.clone()));
    }
    if let Some(context_type) = &options.context_type {
        payload.insert("context_type".to_owned(), context_type.clone());
    }
    if let Some(include_provenance) = options.include_provenance {
        payload.insert(
            "include_provenance".to_owned(),
            Value::Bool(include_provenance),
        );
    }
    if let Some(tags) = &options.tags {
        payload.insert(
            "tags".to_owned(),
            Value::Array(tags.iter().cloned().map(Value::String).collect()),
        );
    }
    if let Some(since) = &options.since {
        payload.insert("since".to_owned(), Value::String(since.clone()));
    }
    if let Some(until) = &options.until {
        payload.insert("until".to_owned(), Value::String(until.clone()));
    }
    if let Some(time_field) = &options.time_field {
        payload.insert("time_field".to_owned(), Value::String(time_field.clone()));
    }
    if let Some(level) = &options.level {
        payload.insert("level".to_owned(), level.to_wire_value());
    }
    if let Some(read_content) = options.read_content {
        payload.insert("read_content".to_owned(), Value::Bool(read_content));
    }
    if let Some(telemetry) = &options.telemetry {
        payload.insert("telemetry".to_owned(), telemetry.clone());
    }
    if let Some(decay) = &options.events_time_decay_protection {
        payload.insert(
            "events_time_decay_protection".to_owned(),
            Value::String(decay.clone()),
        );
    }
    Ok(payload)
}
