// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Virtual filesystem and content operations (`/api/v1/fs/*` and `/api/v1/content/*`).

use http::Method;
use serde_json::{Map, Value};

use crate::client::AsyncHttpClient;
use crate::error::{JsonObject, Result};
use crate::options::{
    compact_request_body, merge_extra, BatchWriteOperation, BatchWriteOptions, LsOptions,
    ReindexOptions, RemoveOptions, SetTagsOptions, TreeOptions, WriteOptions,
};
use crate::transport::{QueryPairs, Transport};
use crate::types::{AclSpec, ListPage, TreePage};
use crate::uri::VikingUri;

impl AsyncHttpClient {
    /// Lists directory entries (`GET /api/v1/fs/ls`).
    pub async fn ls(&self, uri: &str, options: LsOptions) -> Result<Vec<Value>> {
        Ok(self.ls_page(uri, options).await?.result)
    }

    /// Lists directory entries with truncation metadata (`has_more`).
    pub async fn ls_page(&self, uri: &str, options: LsOptions) -> Result<ListPage> {
        let mut query: QueryPairs = vec![
            ("uri".to_owned(), VikingUri::normalize(uri)),
            ("simple".to_owned(), options.simple.to_string()),
            ("recursive".to_owned(), options.recursive.to_string()),
            ("output".to_owned(), options.output),
            ("abs_limit".to_owned(), options.abs_limit.to_string()),
            (
                "show_all_hidden".to_owned(),
                options.show_all_hidden.to_string(),
            ),
            ("node_limit".to_owned(), options.node_limit.to_string()),
            ("offset".to_owned(), options.offset.to_string()),
            (
                "overview_limit".to_owned(),
                options.overview_limit.to_string(),
            ),
        ];
        if let Some(include_abstract) = options.include_abstract {
            query.push(("include_abstract".to_owned(), include_abstract.to_string()));
        }
        if let Some(include_overview) = options.include_overview {
            query.push(("include_overview".to_owned(), include_overview.to_string()));
        }
        if let Some(sort_by) = options.sort_by {
            query.push(("sort_by".to_owned(), sort_by));
            query.push(("sort_order".to_owned(), options.sort_order));
        }
        if let Some(extra_fields) = options.extra_fields {
            for field in extra_fields {
                query.push(("extra_fields".to_owned(), field));
            }
        }
        if let Some(tags) = options.tags {
            for tag in tags {
                query.push(("tags".to_owned(), tag));
            }
        }
        if options.include_tags {
            query.push(("include_tags".to_owned(), "true".to_owned()));
        }
        if let Some(limit) = options.limit {
            query.push(("limit".to_owned(), limit.to_string()));
        }

        let envelope = self
            .transport
            .request_envelope::<Vec<Value>>(Method::GET, "/api/v1/fs/ls", &query, None)
            .await?;
        Ok(ListPage {
            result: envelope.result.unwrap_or_default(),
            has_more: envelope.has_more.unwrap_or(false),
        })
    }

    /// Returns a directory tree (`GET /api/v1/fs/tree`).
    pub async fn tree(&self, uri: &str, options: TreeOptions) -> Result<Vec<JsonObject>> {
        Ok(self.tree_page(uri, options).await?.result)
    }

    /// Returns a directory tree with truncation metadata (`has_more`).
    pub async fn tree_page(&self, uri: &str, options: TreeOptions) -> Result<TreePage> {
        let mut query: QueryPairs = vec![
            ("uri".to_owned(), VikingUri::normalize(uri)),
            ("output".to_owned(), options.output),
            ("abs_limit".to_owned(), options.abs_limit.to_string()),
            (
                "show_all_hidden".to_owned(),
                options.show_all_hidden.to_string(),
            ),
            ("node_limit".to_owned(), options.node_limit.to_string()),
            ("level_limit".to_owned(), options.level_limit.to_string()),
            ("offset".to_owned(), options.offset.to_string()),
            (
                "overview_limit".to_owned(),
                options.overview_limit.to_string(),
            ),
        ];
        if let Some(include_abstract) = options.include_abstract {
            query.push(("include_abstract".to_owned(), include_abstract.to_string()));
        }
        if let Some(include_overview) = options.include_overview {
            query.push(("include_overview".to_owned(), include_overview.to_string()));
        }
        if options.directories_only {
            query.push(("directories_only".to_owned(), "true".to_owned()));
        }
        if let Some(extra_fields) = options.extra_fields {
            for field in extra_fields {
                query.push(("extra_fields".to_owned(), field));
            }
        }
        if let Some(tags) = options.tags {
            for tag in tags {
                query.push(("tags".to_owned(), tag));
            }
        }
        if options.include_tags {
            query.push(("include_tags".to_owned(), "true".to_owned()));
        }
        if let Some(limit) = options.limit {
            query.push(("limit".to_owned(), limit.to_string()));
        }

        let envelope = self
            .transport
            .request_envelope::<Vec<JsonObject>>(Method::GET, "/api/v1/fs/tree", &query, None)
            .await?;
        Ok(TreePage {
            result: envelope.result.unwrap_or_default(),
            has_more: envelope.has_more.unwrap_or(false),
        })
    }

    /// Returns metadata for a `viking://` URI or 32-character hex record ID (`GET /api/v1/fs/stat`).
    pub async fn stat(&self, uri: &str) -> Result<JsonObject> {
        let query: QueryPairs = vec![("uri".to_owned(), VikingUri::normalize_file_reference(uri))];
        self.transport
            .request_result(Method::GET, "/api/v1/fs/stat", &query, None)
            .await
    }

    /// Returns logical extended attributes for a `viking://` URI (`GET /api/v1/fs/attrs`).
    pub async fn attrs(&self, uri: &str) -> Result<JsonObject> {
        let query: QueryPairs = vec![("uri".to_owned(), VikingUri::normalize(uri))];
        self.transport
            .request_result(Method::GET, "/api/v1/fs/attrs", &query, None)
            .await
    }

    /// Creates a directory (`POST /api/v1/fs/mkdir`).
    pub async fn mkdir(
        &self,
        uri: &str,
        description: Option<&str>,
        acl: Option<&AclSpec>,
    ) -> Result<()> {
        let mut payload = Map::new();
        payload.insert("uri".to_owned(), Value::String(VikingUri::normalize(uri)));
        if let Some(desc) = description {
            payload.insert("description".to_owned(), Value::String(desc.to_owned()));
        }
        if let Some(acl_spec) = acl {
            payload.insert("acl".to_owned(), serde_json::to_value(acl_spec)?);
        }
        self.transport
            .request_unit(
                Method::POST,
                "/api/v1/fs/mkdir",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Deletes a file or directory (`DELETE /api/v1/fs`).
    pub async fn rm(&self, uri: &str, options: RemoveOptions) -> Result<()> {
        let query: QueryPairs = vec![
            ("uri".to_owned(), VikingUri::normalize(uri)),
            ("recursive".to_owned(), options.recursive.to_string()),
            ("wait".to_owned(), options.wait.to_string()),
        ];
        self.transport
            .request_unit(Method::DELETE, "/api/v1/fs", &query, None)
            .await
    }

    /// Moves or renames a `viking://` URI (`POST /api/v1/fs/mv`).
    pub async fn mv(&self, from_uri: &str, to_uri: &str) -> Result<()> {
        let mut payload = Map::new();
        payload.insert(
            "from_uri".to_owned(),
            Value::String(VikingUri::normalize(from_uri)),
        );
        payload.insert(
            "to_uri".to_owned(),
            Value::String(VikingUri::normalize(to_uri)),
        );
        self.transport
            .request_unit(
                Method::POST,
                "/api/v1/fs/mv",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Reads UTF-8 text content (`GET /api/v1/content/read`), preserving 32-char hex record IDs.
    pub async fn read(&self, uri: &str, offset: i64, limit: i64) -> Result<String> {
        let query: QueryPairs = vec![
            ("uri".to_owned(), VikingUri::normalize_file_reference(uri)),
            ("offset".to_owned(), offset.to_string()),
            ("limit".to_owned(), limit.to_string()),
        ];
        self.transport
            .request_result(Method::GET, "/api/v1/content/read", &query, None)
            .await
    }

    /// Reads exact stored UTF-8 content including hidden metadata (`GET /api/v1/content/read?raw=true`).
    pub async fn read_raw(&self, uri: &str, offset: i64, limit: i64) -> Result<String> {
        let query: QueryPairs = vec![
            ("uri".to_owned(), VikingUri::normalize_file_reference(uri)),
            ("offset".to_owned(), offset.to_string()),
            ("limit".to_owned(), limit.to_string()),
            ("raw".to_owned(), "true".to_owned()),
        ];
        self.transport
            .request_result(Method::GET, "/api/v1/content/read", &query, None)
            .await
    }

    /// Downloads raw stored bytes without UTF-8 interpretation (`GET /api/v1/content/download`).
    pub async fn download_bytes(&self, uri: &str) -> Result<Vec<u8>> {
        let query: QueryPairs = vec![("uri".to_owned(), VikingUri::normalize(uri))];
        let response = self
            .transport
            .send_raw(Method::GET, "/api/v1/content/download", &query, None)
            .await?;
        if !response.status().is_success() {
            let _: crate::types::ResponseEnvelope<Value> =
                Transport::parse_envelope(response).await?;
            return Ok(Vec::new());
        }
        Ok(response.bytes().await?.to_vec())
    }

    /// Reads L0 `.abstract.md` summary content (`GET /api/v1/content/abstract`).
    pub async fn read_abstract(&self, uri: &str) -> Result<String> {
        let query: QueryPairs = vec![("uri".to_owned(), VikingUri::normalize(uri))];
        self.transport
            .request_result(Method::GET, "/api/v1/content/abstract", &query, None)
            .await
    }

    /// Raw-identifier alias for [`Self::read_abstract`] matching Python's `client.abstract(uri)`.
    pub async fn r#abstract(&self, uri: &str) -> Result<String> {
        self.read_abstract(uri).await
    }

    /// Reads L1 `.overview.md` content (`GET /api/v1/content/overview`).
    pub async fn overview(&self, uri: &str) -> Result<String> {
        let query: QueryPairs = vec![("uri".to_owned(), VikingUri::normalize(uri))];
        self.transport
            .request_result(Method::GET, "/api/v1/content/overview", &query, None)
            .await
    }

    /// Writes text content (including empty string `""` to clear a file) (`POST /api/v1/content/write`).
    pub async fn write(
        &self,
        uri: &str,
        content: &str,
        options: WriteOptions,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("uri".to_owned(), Value::String(VikingUri::normalize(uri)));
        payload.insert("content".to_owned(), Value::String(content.to_owned()));
        payload.insert("mode".to_owned(), Value::String(options.mode));
        payload.insert("wait".to_owned(), Value::Bool(options.wait));

        if let Some(use_snapshot) = options.use_snapshot {
            payload.insert("use_snapshot".to_owned(), Value::Bool(use_snapshot));
        }
        if let Some(acl) = &options.acl {
            payload.insert("acl".to_owned(), serde_json::to_value(acl)?);
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
        if let Some(resolve_conflicts) = options.resolve_conflicts {
            payload.insert(
                "resolve_conflicts".to_owned(),
                Value::Bool(resolve_conflicts),
            );
        }
        if let Some(defer_processing) = options.defer_processing {
            payload.insert("defer_processing".to_owned(), Value::Bool(defer_processing));
        }

        merge_extra(
            &mut payload,
            &options.extra,
            &["uri", "content", "mode", "wait"],
        )?;
        compact_request_body(&mut payload);

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/content/write",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Applies multiple file writes atomically under `root_uri` and refreshes indexes once.
    pub async fn batch_write(
        &self,
        root_uri: &str,
        operations: &[BatchWriteOperation],
        options: BatchWriteOptions,
    ) -> Result<JsonObject> {
        let mut normalized_ops = Vec::with_capacity(operations.len());
        for op in operations {
            let mut copy = op.clone();
            copy.uri = VikingUri::normalize(&copy.uri);
            normalized_ops.push(serde_json::to_value(copy)?);
        }

        let mut payload = Map::new();
        payload.insert(
            "root_uri".to_owned(),
            Value::String(VikingUri::normalize(root_uri)),
        );
        payload.insert("operations".to_owned(), Value::Array(normalized_ops));
        payload.insert("wait".to_owned(), Value::Bool(options.wait));

        if let Some(use_snapshot) = options.use_snapshot {
            payload.insert("use_snapshot".to_owned(), Value::Bool(use_snapshot));
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
        if let Some(skip_conflicts) = options.skip_conflicts {
            payload.insert("skip_conflicts".to_owned(), Value::Bool(skip_conflicts));
        }
        if let Some(defer_processing) = options.defer_processing {
            payload.insert("defer_processing".to_owned(), Value::Bool(defer_processing));
        }

        merge_extra(
            &mut payload,
            &options.extra,
            &["root_uri", "operations", "wait"],
        )?;
        compact_request_body(&mut payload);

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/content/batch-write",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Ensures L0/L1 directory index summaries exist (`POST /api/v1/content/ensure_directory_index`).
    pub async fn ensure_directory_index(
        &self,
        uri: &str,
        fallback_abstract: &str,
        wait: bool,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("uri".to_owned(), Value::String(VikingUri::normalize(uri)));
        payload.insert(
            "fallback_abstract".to_owned(),
            Value::String(fallback_abstract.to_owned()),
        );
        payload.insert("wait".to_owned(), Value::Bool(wait));

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/content/ensure_directory_index",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Updates retrieval tags on a single file or directory (`POST /api/v1/fs/attrs/set_tags`).
    pub async fn set_tags(
        &self,
        uri: &str,
        tags: Option<&[String]>,
        options: SetTagsOptions,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("uri".to_owned(), Value::String(VikingUri::normalize(uri)));
        payload.insert(
            "mode".to_owned(),
            Value::String(options.mode.as_str().to_owned()),
        );
        payload.insert("recursive".to_owned(), Value::Bool(options.recursive));
        if let Some(tag_list) = tags {
            payload.insert(
                "tags".to_owned(),
                Value::Array(tag_list.iter().cloned().map(Value::String).collect()),
            );
        }
        if let Some(telemetry) = &options.telemetry {
            payload.insert("telemetry".to_owned(), telemetry.clone());
        }
        merge_extra(
            &mut payload,
            &options.extra,
            &["uri", "tags", "mode", "recursive"],
        )?;
        compact_request_body(&mut payload);

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/fs/attrs/set_tags",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Updates retrieval tags across multiple URIs (`POST /api/v1/content/batch_set_tags`).
    pub async fn batch_set_tags(
        &self,
        uris: &[String],
        tags: Option<&[String]>,
        options: SetTagsOptions,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert(
            "uris".to_owned(),
            Value::Array(
                uris.iter()
                    .map(|u| Value::String(VikingUri::normalize(u)))
                    .collect(),
            ),
        );
        payload.insert(
            "mode".to_owned(),
            Value::String(options.mode.as_str().to_owned()),
        );
        payload.insert("recursive".to_owned(), Value::Bool(options.recursive));
        if let Some(tag_list) = tags {
            payload.insert(
                "tags".to_owned(),
                Value::Array(tag_list.iter().cloned().map(Value::String).collect()),
            );
        }
        if let Some(telemetry) = &options.telemetry {
            payload.insert("telemetry".to_owned(), telemetry.clone());
        }
        merge_extra(
            &mut payload,
            &options.extra,
            &["uris", "tags", "mode", "recursive"],
        )?;
        compact_request_body(&mut payload);

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/content/batch_set_tags",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Reindexes content under a `viking://` URI (`POST /api/v1/content/reindex`).
    pub async fn reindex(&self, uri: &str, options: ReindexOptions) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("uri".to_owned(), Value::String(VikingUri::normalize(uri)));
        payload.insert("mode".to_owned(), Value::String(options.mode));
        payload.insert("wait".to_owned(), Value::Bool(options.wait));
        payload.insert("recursive".to_owned(), Value::Bool(options.recursive));
        if options.force {
            payload.insert("force".to_owned(), Value::Bool(true));
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
            &["uri", "mode", "wait", "recursive", "force"],
        )?;
        compact_request_body(&mut payload);

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/content/reindex",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }
}
