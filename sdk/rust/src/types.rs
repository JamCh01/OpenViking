// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Shared domain types and response structures for the OpenViking HTTP SDK.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{ApiErrorPayload, JsonObject};
use crate::uri::VikingUri;

/// Tri-state field distinguishing omitted (`Unset`), explicit JSON `null` (`Null`), and a concrete value.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Nullable<T> {
    /// Omit the field from the outgoing JSON payload.
    #[default]
    Unset,
    /// Send an explicit JSON `null` (for example to disable `auto_commit_policy`).
    Null,
    /// Send the provided value.
    Value(T),
}

impl<T> From<T> for Nullable<T> {
    fn from(value: T) -> Self {
        Self::Value(value)
    }
}

impl<T> From<Option<T>> for Nullable<T> {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(value) => Self::Value(value),
            None => Self::Null,
        }
    }
}

/// One target `viking://` URI or a list of target scope URIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetUri {
    /// Single URI scope.
    Single(String),
    /// Multiple URI scopes.
    Multiple(Vec<String>),
}

impl Default for TargetUri {
    fn default() -> Self {
        Self::Single(String::new())
    }
}

impl From<&str> for TargetUri {
    fn from(uri: &str) -> Self {
        Self::Single(uri.to_owned())
    }
}

impl From<String> for TargetUri {
    fn from(uri: String) -> Self {
        Self::Single(uri)
    }
}

impl From<Vec<String>> for TargetUri {
    fn from(uris: Vec<String>) -> Self {
        Self::Multiple(uris)
    }
}

impl From<&[&str]> for TargetUri {
    fn from(uris: &[&str]) -> Self {
        Self::Multiple(uris.iter().map(|s| (*s).to_owned()).collect())
    }
}

impl<const N: usize> From<[&str; N]> for TargetUri {
    fn from(uris: [&str; N]) -> Self {
        Self::Multiple(uris.into_iter().map(str::to_owned).collect())
    }
}

impl TargetUri {
    /// Normalizes every non-empty URI in this target into canonical `viking://` form.
    pub fn to_normalized_value(&self) -> Value {
        match self {
            Self::Single(uri) => {
                if uri.is_empty() {
                    Value::String(String::new())
                } else {
                    Value::String(VikingUri::normalize(uri))
                }
            }
            Self::Multiple(uris) => Value::Array(
                uris.iter()
                    .map(|u| {
                        if u.is_empty() {
                            Value::String(String::new())
                        } else {
                            Value::String(VikingUri::normalize(u))
                        }
                    })
                    .collect(),
            ),
        }
    }
}

/// Retrieval level selector (`0`, `"abstract"`, or `[0, 1]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LevelFilter {
    /// Single numeric level (`0`, `1`, `2`).
    Single(i64),
    /// Named level string.
    Named(String),
    /// Multiple numeric levels.
    Multiple(Vec<i64>),
}

impl From<i64> for LevelFilter {
    fn from(level: i64) -> Self {
        Self::Single(level)
    }
}

impl From<&str> for LevelFilter {
    fn from(name: &str) -> Self {
        Self::Named(name.to_owned())
    }
}

impl From<String> for LevelFilter {
    fn from(name: String) -> Self {
        Self::Named(name)
    }
}

impl From<Vec<i64>> for LevelFilter {
    fn from(levels: Vec<i64>) -> Self {
        Self::Multiple(levels)
    }
}

impl LevelFilter {
    /// Converts this level filter into its JSON wire representation.
    pub fn to_wire_value(&self) -> Value {
        match self {
            Self::Single(level) => Value::Number((*level).into()),
            Self::Named(name) => Value::String(name.clone()),
            Self::Multiple(levels) => Value::Array(
                levels
                    .iter()
                    .copied()
                    .map(|n| Value::Number(n.into()))
                    .collect(),
            ),
        }
    }
}

/// Post-ingest resource processing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessingMode {
    /// Generate semantic L0/L1 summaries and vector embeddings.
    #[serde(rename = "semantic_and_vectors")]
    SemanticAndVectors,
    /// Vectorize content without VLM semantic summary generation.
    #[serde(rename = "vectors_only")]
    VectorsOnly,
}

impl ProcessingMode {
    /// Returns the wire string for this processing mode.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SemanticAndVectors => "semantic_and_vectors",
            Self::VectorsOnly => "vectors_only",
        }
    }
}

/// Tag mutation mode for `set_tags`, `batch_set_tags`, `write`, `add_resource`, and `reindex`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TagMode {
    /// Replace existing tags with the provided list.
    #[default]
    #[serde(rename = "replace")]
    Replace,
    /// Append or update provided `k=v` / counter tags.
    #[serde(rename = "append")]
    Append,
    /// Clear all existing tags on the target.
    #[serde(rename = "clear")]
    Clear,
}

impl TagMode {
    /// Returns the wire string for this tag mode.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Append => "append",
            Self::Clear => "clear",
        }
    }
}

/// Output format accepted by observer endpoints (`/api/v1/observer/*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObserverFormat {
    /// Formatted table representation.
    #[serde(rename = "table")]
    Table,
    /// Structured JSON representation.
    #[serde(rename = "json")]
    Json,
}

impl ObserverFormat {
    /// Returns the query-parameter string for this observer format.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::Json => "json",
        }
    }
}

/// One direct ACL grant entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AclEntry {
    /// User or group principal identifier.
    pub principal: String,
    /// Access level (`"read"`, `"write"`, or `"manage"`).
    pub level: String,
}

impl AclEntry {
    /// Creates a new [`AclEntry`].
    pub fn new(principal: impl Into<String>, level: impl Into<String>) -> Self {
        Self {
            principal: principal.into(),
            level: level.into(),
        }
    }
}

/// Writable ACL specification (`entries` and/or `acl_mode`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AclSpec {
    /// Optional inheritance mode (`"inherit"` or `"restricted"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acl_mode: Option<String>,
    /// Optional direct ACL grant entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<Vec<AclEntry>>,
}

/// One MCP tool live-context mount specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolMount {
    /// Registered MCP tool URI (`viking://agent/tools/<server>/<tool>.json`).
    pub tool_uri: String,
    /// Destination live-context resource URI (`viking://resources/.../*.md`).
    pub node_uri: String,
    /// Human-readable live-context title.
    pub title: String,
    /// Optional live-context description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Execution timeout in seconds (defaults to 60 when omitted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<i64>,
    /// Tool arguments passed when evaluating the live-context node.
    #[serde(default)]
    pub arguments: JsonObject,
    /// Optional retrieval tags attached to the mounted node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

impl McpToolMount {
    /// Creates a new [`McpToolMount`] with required fields.
    pub fn new(
        tool_uri: impl Into<String>,
        node_uri: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        Self {
            tool_uri: tool_uri.into(),
            node_uri: node_uri.into(),
            title: title.into(),
            description: None,
            timeout_seconds: None,
            arguments: JsonObject::new(),
            tags: None,
        }
    }
}

/// Paginated directory listing returned by `ls_page`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListPage {
    /// Listed entries (`String` names in simple mode or `JsonObject` items in full mode).
    #[serde(default)]
    pub result: Vec<Value>,
    /// Whether the server truncated additional entries due to `limit` or `node_limit`.
    #[serde(default)]
    pub has_more: bool,
}

/// Paginated directory tree returned by `tree_page`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TreePage {
    /// Tree node objects.
    #[serde(default)]
    pub result: Vec<JsonObject>,
    /// Whether the server truncated additional nodes due to `limit` or `node_limit`.
    #[serde(default)]
    pub has_more: bool,
}

/// One assembled context entry returned by `search_context`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SearchContextEntry {
    /// Canonical `viking://` URI.
    #[serde(default)]
    pub uri: Option<String>,
    /// Context category (`"memory"`, `"resource"`, `"skill"`).
    #[serde(default)]
    pub category: Option<String>,
    /// Relevance score.
    #[serde(default)]
    pub score: Option<f64>,
    /// Selected detail tier (`"abstract"`, `"overview"`, `"full"`).
    #[serde(default)]
    pub detail: Option<String>,
    /// Expanded text snippet.
    #[serde(default)]
    pub text: Option<String>,
    /// Provenance/origin descriptor.
    #[serde(default)]
    pub origin: Option<String>,
    /// Additional server fields.
    #[serde(flatten)]
    pub extra: JsonObject,
}

/// Server-assembled context block returned by `search_context`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SearchContextResult {
    /// Ranked context entries included in the assembled block.
    #[serde(default)]
    pub entries: Vec<SearchContextEntry>,
    /// Rendered context string ready for prompt injection.
    #[serde(default)]
    pub rendered: Option<String>,
    /// Content digest of the rendered block.
    #[serde(default)]
    pub digest: Option<String>,
    /// Token/category assembly statistics.
    #[serde(default)]
    pub stats: Option<JsonObject>,
    /// Additional server fields.
    #[serde(flatten)]
    pub extra: JsonObject,
}

/// Binary blob returned when `git_show` is called with a file `path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotBlob {
    /// Snapshot object ID from `X-Snapshot-Oid`.
    pub oid: String,
    /// Blob byte length from `X-Snapshot-Size`.
    pub size: u64,
    /// Raw file bytes.
    pub bytes: Vec<u8>,
}

/// Polymorphic return value of `git_show` (`path = None` returns commit JSON, `path = Some(...)` returns a blob).
#[derive(Debug, Clone, PartialEq)]
pub enum SnapshotShowResult {
    /// Commit metadata object when `path` was omitted.
    Commit(JsonObject),
    /// Binary file blob when `path` was specified.
    Blob(SnapshotBlob),
}

/// Standard OpenViking HTTP JSON response envelope.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResponseEnvelope<T = Value> {
    /// Envelope status (`"ok"` or `"error"`).
    #[serde(default)]
    pub status: Option<String>,
    /// Unwrapped endpoint result payload.
    #[serde(default)]
    pub result: Option<T>,
    /// Pagination truncation indicator on `ls` / `tree` endpoints.
    #[serde(default)]
    pub has_more: Option<bool>,
    /// Structured error payload when `status == "error"`.
    #[serde(default)]
    pub error: Option<ApiErrorPayload>,
    /// FastAPI/HTTP fallback error detail when `error` is absent.
    #[serde(default)]
    pub detail: Option<Value>,
    /// Optional server telemetry payload.
    #[serde(default)]
    pub telemetry: Option<Value>,
    /// Optional server profiling trace lines when `?profile=1` is enabled.
    #[serde(default)]
    pub profile: Option<Vec<String>>,
}
