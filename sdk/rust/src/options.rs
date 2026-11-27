// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Typed request options and forward-compatible `extra` payload builders.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{JsonObject, OpenVikingError, Result};
use crate::types::{AclSpec, LevelFilter, Nullable, ProcessingMode, TagMode, TargetUri};

/// Multimodal image input accepted by `find`, `search`, and `search_context`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageInput {
    /// Local file path, `viking://` URI, `http(s)://` URL, or `data:image/...` URI.
    Reference(String),
    /// Raw image bytes with an optional filename hint for MIME detection.
    Bytes {
        /// Raw image bytes.
        data: Vec<u8>,
        /// Optional filename (e.g. `"photo.jpg"`) used to infer MIME type.
        file_name: Option<String>,
    },
    /// Multiple image inputs.
    Multiple(Vec<ImageInput>),
}

impl From<&str> for ImageInput {
    fn from(value: &str) -> Self {
        Self::Reference(value.to_owned())
    }
}

impl From<String> for ImageInput {
    fn from(value: String) -> Self {
        Self::Reference(value)
    }
}

impl From<&Path> for ImageInput {
    fn from(path: &Path) -> Self {
        Self::Reference(path.to_string_lossy().into_owned())
    }
}

impl From<PathBuf> for ImageInput {
    fn from(path: PathBuf) -> Self {
        Self::Reference(path.to_string_lossy().into_owned())
    }
}

impl From<Vec<u8>> for ImageInput {
    fn from(data: Vec<u8>) -> Self {
        Self::Bytes {
            data,
            file_name: None,
        }
    }
}

impl From<&[u8]> for ImageInput {
    fn from(data: &[u8]) -> Self {
        Self::Bytes {
            data: data.to_vec(),
            file_name: None,
        }
    }
}

/// Options for semantic `find` queries (`/api/v1/search/find`).
#[derive(Debug, Clone, Default)]
pub struct FindOptions {
    /// Target URI or list of target URIs.
    pub target_uri: Option<TargetUri>,
    /// Maximum number of results to return (defaults to 10).
    pub limit: Option<usize>,
    /// Optional image query input.
    pub image: Option<ImageInput>,
    /// Optional candidate node scan limit.
    pub node_limit: Option<usize>,
    /// Minimum similarity score threshold.
    pub score_threshold: Option<f64>,
    /// Structured metadata filter expression.
    pub filter: Option<JsonObject>,
    /// Context category filter (`"memory"`, `"resource"`, `"skill"`, or array).
    pub context_type: Option<Value>,
    /// Whether provenance fields are included.
    pub include_provenance: Option<bool>,
    /// Tag filter list.
    pub tags: Option<Vec<String>>,
    /// Lower time bound (ISO-8601).
    pub since: Option<String>,
    /// Upper time bound (ISO-8601).
    pub until: Option<String>,
    /// Timestamp field (`"updated_at"` or `"created_at"`).
    pub time_field: Option<String>,
    /// Content level selector (`0`, `1`, `2`, or list).
    pub level: Option<LevelFilter>,
    /// Whether stored text content is expanded inline.
    pub read_content: Option<bool>,
    /// Optional telemetry configuration.
    pub telemetry: Option<Value>,
    /// Optional event time-decay protection setting.
    pub events_time_decay_protection: Option<String>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

/// Options for session-aware `search` queries (`/api/v1/search/search`).
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    /// Optional session ID providing conversational context.
    pub session_id: Option<String>,
    /// Shared semantic retrieval options.
    pub find: FindOptions,
    /// Search engine type (`"semantic"` or `"keywords"`).
    pub search_type: Option<String>,
    /// Engine-specific arguments (forwarded only when non-empty).
    pub args: Option<JsonObject>,
}

/// Options for server-side `search_context` assembly (`/api/v1/search/search` with `mode="context"`).
#[derive(Debug, Clone, Default)]
pub struct SearchContextOptions {
    /// Optional session ID.
    pub session_id: Option<String>,
    /// Target URI or list of target URIs.
    pub target_uri: Option<TargetUri>,
    /// Result limit (defaults to 10).
    pub limit: Option<usize>,
    /// Search type (`"semantic"` or `"keywords"`).
    pub search_type: Option<String>,
    /// Optional image input.
    pub image: Option<ImageInput>,
    /// Candidate node scan limit.
    pub node_limit: Option<usize>,
    /// Minimum score threshold.
    pub score_threshold: Option<f64>,
    /// Metadata filter expression.
    pub filter: Option<JsonObject>,
    /// Context category filter.
    pub context_type: Option<Value>,
    /// Whether provenance is included.
    pub include_provenance: Option<bool>,
    /// Retrieval tag filters.
    pub tags: Option<Vec<String>>,
    /// Lower time bound.
    pub since: Option<String>,
    /// Upper time bound.
    pub until: Option<String>,
    /// Time field (`"updated_at"` or `"created_at"`).
    pub time_field: Option<String>,
    /// Event time-decay protection mode.
    pub events_time_decay_protection: Option<String>,
    /// Query expansion mode (`"off"` or `"auto"`).
    pub query_expansion: Option<String>,
    /// Token budget for assembled context.
    pub max_tokens: Option<usize>,
    /// Per-category token quotas.
    pub quotas: Option<BTreeMap<String, usize>>,
    /// Preset purpose (`"chat"` or `"coding"`).
    pub purpose: Option<String>,
    /// Detail tier (`"auto"`, `"abstract"`, `"overview"`, `"full"`, or per-category map).
    pub detail: Option<Value>,
    /// Cross-turn deduplication window.
    pub dedup_turns: Option<usize>,
    /// URIs excluded from assembly.
    pub exclude_uris: Option<Vec<String>>,
    /// Peer visibility scope (`"actor"` or `"all"`).
    pub peer_scope: Option<String>,
    /// Score penalty applied to memories owned by other peers.
    pub other_peer_penalty: Option<Value>,
    /// Rewrite setting (`bool` or `"auto"`).
    pub rewrite: Option<Value>,
    /// Maximum bullet count when rewriting.
    pub rewrite_max_bullets: Option<usize>,
    /// Optional telemetry configuration.
    pub telemetry: Option<Value>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

/// Options for `grep` content search (`/api/v1/search/grep`).
#[derive(Debug, Clone)]
pub struct GrepOptions {
    /// Case-insensitive regex/substring match.
    pub case_insensitive: bool,
    /// Maximum number of nodes scanned (defaults to 256).
    pub node_limit: usize,
    /// Optional subtree URI to exclude.
    pub exclude_uri: Option<String>,
    /// Optional tag filters.
    pub tags: Option<Vec<String>>,
    /// Include node tags in matches.
    pub include_tags: bool,
    /// Optional directory depth limit.
    pub level_limit: Option<usize>,
}

impl Default for GrepOptions {
    fn default() -> Self {
        Self {
            case_insensitive: false,
            node_limit: 256,
            exclude_uri: None,
            tags: None,
            include_tags: false,
            level_limit: None,
        }
    }
}

/// Options for `glob` path matching (`/api/v1/search/glob`).
#[derive(Debug, Clone)]
pub struct GlobOptions {
    /// Root URI(s) to search (defaults to `"viking://"`).
    pub uri: TargetUri,
    /// Maximum number of nodes scanned (defaults to 256).
    pub node_limit: usize,
    /// Optional extra metadata fields (`"locked"`, `"id"`, `"count"`).
    pub extra_fields: Option<Vec<String>>,
    /// Optional tag filters.
    pub tags: Option<Vec<String>>,
    /// Include node tags in results.
    pub include_tags: bool,
}

impl Default for GlobOptions {
    fn default() -> Self {
        Self {
            uri: TargetUri::from("viking://"),
            node_limit: 256,
            extra_fields: None,
            tags: None,
            include_tags: false,
        }
    }
}

/// Options for `ls` and `ls_page` (`/api/v1/fs/ls`).
#[derive(Debug, Clone)]
pub struct LsOptions {
    /// Return simple entry names instead of full metadata objects.
    pub simple: bool,
    /// Traverse subdirectories recursively.
    pub recursive: bool,
    /// Output projection (`"original"` or `"agent"`).
    pub output: String,
    /// Character limit for L0 abstracts (defaults to 256).
    pub abs_limit: usize,
    /// Include hidden entries such as `.abstract.md`.
    pub show_all_hidden: bool,
    /// Maximum node scan count (defaults to 1000).
    pub node_limit: usize,
    /// Optional sort field (`"name"` or `"mtime"`).
    pub sort_by: Option<String>,
    /// Sort direction when `sort_by` is set (defaults to `"asc"`).
    pub sort_order: String,
    /// Optional extra metadata fields.
    pub extra_fields: Option<Vec<String>>,
    /// Optional tag filter list.
    pub tags: Option<Vec<String>>,
    /// Include tags on returned entries.
    pub include_tags: bool,
    /// Pagination offset (defaults to 0).
    pub offset: usize,
    /// Optional page size limit.
    pub limit: Option<usize>,
    /// Explicitly include/exclude `.abstract.md` text.
    pub include_abstract: Option<bool>,
    /// Explicitly include/exclude `.overview.md` text.
    pub include_overview: Option<bool>,
    /// Character limit for L1 overviews (defaults to 4000).
    pub overview_limit: usize,
}

impl Default for LsOptions {
    fn default() -> Self {
        Self {
            simple: false,
            recursive: false,
            output: "original".to_owned(),
            abs_limit: 256,
            show_all_hidden: false,
            node_limit: 1000,
            sort_by: None,
            sort_order: "asc".to_owned(),
            extra_fields: None,
            tags: None,
            include_tags: false,
            offset: 0,
            limit: None,
            include_abstract: None,
            include_overview: None,
            overview_limit: 4000,
        }
    }
}

/// Options for `tree` and `tree_page` (`/api/v1/fs/tree`).
#[derive(Debug, Clone)]
pub struct TreeOptions {
    /// Output projection (`"original"` or `"agent"`).
    pub output: String,
    /// Character limit for L0 abstracts (defaults to 128).
    pub abs_limit: usize,
    /// Include hidden entries.
    pub show_all_hidden: bool,
    /// Maximum node scan count (defaults to 1000).
    pub node_limit: usize,
    /// Maximum recursion depth (defaults to 3).
    pub level_limit: usize,
    /// Optional extra metadata fields.
    pub extra_fields: Option<Vec<String>>,
    /// Optional tag filter list.
    pub tags: Option<Vec<String>>,
    /// Include tags on returned nodes.
    pub include_tags: bool,
    /// Pagination offset (defaults to 0).
    pub offset: usize,
    /// Optional page size limit.
    pub limit: Option<usize>,
    /// Explicitly include/exclude `.abstract.md` text.
    pub include_abstract: Option<bool>,
    /// Explicitly include/exclude `.overview.md` text.
    pub include_overview: Option<bool>,
    /// Character limit for L1 overviews (defaults to 4000).
    pub overview_limit: usize,
    /// Return directories only.
    pub directories_only: bool,
}

impl Default for TreeOptions {
    fn default() -> Self {
        Self {
            output: "original".to_owned(),
            abs_limit: 128,
            show_all_hidden: false,
            node_limit: 1000,
            level_limit: 3,
            extra_fields: None,
            tags: None,
            include_tags: false,
            offset: 0,
            limit: None,
            include_abstract: None,
            include_overview: None,
            overview_limit: 4000,
            directories_only: false,
        }
    }
}

/// Options for `rm` (`/api/v1/fs`).
#[derive(Debug, Clone, Copy, Default)]
pub struct RemoveOptions {
    /// Remove directories recursively.
    pub recursive: bool,
    /// Wait for background index cleanup to finish.
    pub wait: bool,
}

/// Options for `add_resource` (`/api/v1/resources`).
#[derive(Debug, Clone, Default)]
pub struct AddResourceOptions {
    /// Exact destination `viking://` URI (mutually exclusive with `parent`).
    pub to: Option<String>,
    /// Parent directory `viking://` URI (mutually exclusive with `to` and `add_type`).
    pub parent: Option<String>,
    /// Wait for resource processing to complete before returning.
    pub wait: bool,
    /// Initial ACL specification.
    pub acl: Option<AclSpec>,
    /// Reason why this resource is being imported.
    pub reason: Option<String>,
    /// Custom semantic extraction instruction.
    pub instruction: Option<String>,
    /// Create missing parent directories automatically.
    pub create_parent: Option<bool>,
    /// Enforce strict parser errors.
    pub strict: Option<bool>,
    /// Comma-separated directory names ignored during import.
    pub ignore_dirs: Option<String>,
    /// Glob include pattern.
    pub include: Option<String>,
    /// Glob exclude pattern.
    pub exclude: Option<String>,
    /// Upload media files directly without transcoding.
    pub directly_upload_media: Option<bool>,
    /// Preserve source directory hierarchy.
    pub preserve_structure: Option<bool>,
    /// Periodic watch interval in minutes/seconds (`> 0` creates a watch task).
    pub watch_interval: Option<f64>,
    /// Connector-specific arguments (omitted when empty).
    pub args: Option<JsonObject>,
    /// Optional telemetry configuration.
    pub telemetry: Option<Value>,
    /// Post-ingest processing mode (`semantic_and_vectors` or `vectors_only`).
    pub processing_mode: Option<ProcessingMode>,
    /// External connector type (e.g. `"feishu"`; requires `to` and forbids `parent`).
    pub add_type: Option<String>,
    /// Retrieval tags attached to imported nodes.
    pub tags: Option<Vec<String>>,
    /// Tag mutation mode (`replace`, `append`, `clear`).
    pub tag_mode: Option<TagMode>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

/// Skill payload accepted by `add_skill`, `update_skill`, and `validate_skill`.
#[derive(Debug, Clone)]
pub enum SkillSource {
    /// Local file/directory path (uploaded automatically if it exists) or inline Markdown text.
    TextOrPath(String),
    /// Structured JSON skill object.
    Json(Value),
}

impl From<&str> for SkillSource {
    fn from(s: &str) -> Self {
        Self::TextOrPath(s.to_owned())
    }
}

impl From<String> for SkillSource {
    fn from(s: String) -> Self {
        Self::TextOrPath(s)
    }
}

impl From<&Path> for SkillSource {
    fn from(path: &Path) -> Self {
        Self::TextOrPath(path.to_string_lossy().into_owned())
    }
}

impl From<PathBuf> for SkillSource {
    fn from(path: PathBuf) -> Self {
        Self::TextOrPath(path.to_string_lossy().into_owned())
    }
}

impl From<Value> for SkillSource {
    fn from(value: Value) -> Self {
        Self::Json(value)
    }
}

impl From<JsonObject> for SkillSource {
    fn from(map: JsonObject) -> Self {
        Self::Json(Value::Object(map))
    }
}

/// Options for `add_skill` (`/api/v1/skills`).
#[derive(Debug, Clone, Default)]
pub struct AddSkillOptions {
    /// Wait for indexing to complete before returning.
    pub wait: bool,
    /// Optional telemetry configuration.
    pub telemetry: Option<Value>,
    /// Target skill root URI (e.g. `"viking://~/skills"` or `"viking://agent/skills"`).
    pub target_uri: Option<String>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

/// Options for `update_skill` (`PUT /api/v1/skills/{skill_name}`).
#[derive(Debug, Clone, Default)]
pub struct UpdateSkillOptions {
    /// Wait for indexing to complete before returning.
    pub wait: bool,
    /// Optional telemetry configuration.
    pub telemetry: Option<Value>,
    /// Target skill root URI.
    pub target_uri: Option<String>,
    /// Optional source provenance metadata.
    pub source_metadata: Option<JsonObject>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

/// Options for `get_skill` (`GET /api/v1/skills/{skill_name}`).
#[derive(Debug, Clone)]
pub struct GetSkillOptions {
    /// Explicitly include/exclude SKILL.md body content.
    pub include_content: Option<bool>,
    /// Include companion file manifest (defaults to `true`).
    pub include_files: bool,
    /// Include installation source metadata (defaults to `false`).
    pub include_source: bool,
    /// Optional content level (`0`, `1`, `2`).
    pub level: Option<i64>,
    /// Optional skill space URI.
    pub target_uri: Option<String>,
    /// Include file integrity hashes (defaults to `false`).
    pub include_integrity: bool,
}

impl Default for GetSkillOptions {
    fn default() -> Self {
        Self {
            include_content: None,
            include_files: true,
            include_source: false,
            level: None,
            target_uri: None,
            include_integrity: false,
        }
    }
}

/// Options for `find_skills` (`/api/v1/skills/find`).
#[derive(Debug, Clone)]
pub struct FindSkillsOptions {
    /// Maximum results (defaults to 10).
    pub limit: usize,
    /// Minimum similarity score threshold.
    pub score_threshold: Option<f64>,
    /// Optional level list.
    pub level: Option<Vec<i64>>,
    /// Telemetry flag or object (defaults to `Value::Bool(false)` to match Python SDK).
    pub telemetry: Value,
    /// Optional skill space URI.
    pub target_uri: Option<String>,
}

impl Default for FindSkillsOptions {
    fn default() -> Self {
        Self {
            limit: 10,
            score_threshold: None,
            level: None,
            telemetry: Value::Bool(false),
            target_uri: None,
        }
    }
}

/// Options for `validate_skill` (`/api/v1/skills/validate`).
#[derive(Debug, Clone, Default)]
pub struct ValidateSkillOptions {
    /// Enable strict lint rules.
    pub strict: bool,
    /// Optional original source path hint.
    pub source_path: Option<String>,
    /// Optional directory name hint.
    pub skill_dir_name: Option<String>,
    /// Optional skill space URI.
    pub target_uri: Option<String>,
}

/// Options for `write` (`/api/v1/content/write`).
#[derive(Debug, Clone)]
pub struct WriteOptions {
    /// Write mode (`"replace"`, `"append"`, `"create"`, `"upsert"`; defaults to `"replace"`).
    pub mode: String,
    /// Wait for background semantic/vector refresh to complete.
    pub wait: bool,
    /// Record write through snapshot control when enabled.
    pub use_snapshot: Option<bool>,
    /// Optional ACL specification.
    pub acl: Option<AclSpec>,
    /// Optional telemetry configuration.
    pub telemetry: Option<Value>,
    /// Optional processing mode (`semantic_and_vectors` or `vectors_only`).
    pub processing_mode: Option<ProcessingMode>,
    /// Optional tags to set alongside the write.
    pub tags: Option<Vec<String>>,
    /// Tag mutation mode (`replace`, `append`, `clear`).
    pub tag_mode: Option<TagMode>,
    /// Resolve snapshot/target conflicts automatically.
    pub resolve_conflicts: Option<bool>,
    /// Save content without scheduling semantic or vector indexing.
    pub defer_processing: Option<bool>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            mode: "replace".to_owned(),
            wait: false,
            use_snapshot: None,
            acl: None,
            telemetry: None,
            processing_mode: None,
            tags: None,
            tag_mode: None,
            resolve_conflicts: None,
            defer_processing: None,
            extra: JsonObject::new(),
        }
    }
}

/// Single file write operation inside `batch_write`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatchWriteOperation {
    /// Target file `viking://` URI (normalized automatically before submission).
    pub uri: String,
    /// UTF-8 text content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Base64-encoded binary content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_base64: Option<String>,
    /// Write mode (`"replace"`, `"append"`, `"create"`, `"upsert"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Optimistic concurrency SHA-256 precondition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_sha256: Option<String>,
    /// Write only when the target file does not yet exist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub if_absent: Option<bool>,
    /// Resolve target conflicts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolve_conflicts: Option<bool>,
    /// Optional tags for this file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Tag mutation mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_mode: Option<TagMode>,
    /// Optional ACL specification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acl: Option<AclSpec>,
}

impl BatchWriteOperation {
    /// Creates a UTF-8 text write operation for `uri`.
    pub fn text(uri: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            content: Some(content.into()),
            ..Self::default()
        }
    }
}

/// Options for `batch_write` (`/api/v1/content/batch-write`).
#[derive(Debug, Clone)]
pub struct BatchWriteOptions {
    /// Wait for post-batch index refresh to finish (defaults to `true` to match Python SDK).
    pub wait: bool,
    /// Record writes through snapshot control.
    pub use_snapshot: Option<bool>,
    /// Optional telemetry configuration.
    pub telemetry: Option<Value>,
    /// Optional processing mode.
    pub processing_mode: Option<ProcessingMode>,
    /// Skip conflicting operations without failing compatible writes.
    pub skip_conflicts: Option<bool>,
    /// Save content without scheduling semantic or vector work.
    pub defer_processing: Option<bool>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

impl Default for BatchWriteOptions {
    fn default() -> Self {
        Self {
            wait: true,
            use_snapshot: None,
            telemetry: None,
            processing_mode: None,
            skip_conflicts: None,
            defer_processing: None,
            extra: JsonObject::new(),
        }
    }
}

/// Options for `set_tags` and `batch_set_tags`.
#[derive(Debug, Clone, Default)]
pub struct SetTagsOptions {
    /// Tag update mode (`replace` by default, `append`, or `clear`).
    pub mode: TagMode,
    /// Apply recursively when targeting a directory.
    pub recursive: bool,
    /// Optional telemetry configuration.
    pub telemetry: Option<Value>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

/// Options for `reindex` (`/api/v1/content/reindex`).
#[derive(Debug, Clone)]
pub struct ReindexOptions {
    /// Processing mode (defaults to `"vectors_only"`).
    pub mode: String,
    /// Wait for completion (defaults to `true`).
    pub wait: bool,
    /// Recurse into subdirectories (defaults to `true`).
    pub recursive: bool,
    /// Force reindexing even if hashes match (defaults to `false`; omitted from body when `false`).
    pub force: bool,
    /// Optional tags updated alongside reindexing.
    pub tags: Option<Vec<String>>,
    /// Tag mutation mode.
    pub tag_mode: Option<TagMode>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

impl Default for ReindexOptions {
    fn default() -> Self {
        Self {
            mode: "vectors_only".to_owned(),
            wait: true,
            recursive: true,
            force: false,
            tags: None,
            tag_mode: None,
            extra: JsonObject::new(),
        }
    }
}

/// Options for `create_session` (`/api/v1/sessions`).
#[derive(Debug, Clone, Default)]
pub struct CreateSessionOptions {
    /// Explicit session ID (server generates one when omitted).
    pub session_id: Option<String>,
    /// Session memory policy configuration.
    pub memory_policy: Option<JsonObject>,
    /// Auto-commit policy (`Nullable::Null` explicitly disables server-wide auto-commit).
    pub auto_commit_policy: Nullable<JsonObject>,
    /// Event/memory extraction configuration.
    pub memory_extraction_config: Option<JsonObject>,
    /// Optional telemetry configuration.
    pub telemetry: Option<Value>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

/// Options for `update_session_config` (`PATCH /api/v1/sessions/{session_id}/config`).
#[derive(Debug, Clone, Default)]
pub struct UpdateSessionConfigOptions {
    /// Auto-commit policy (`Nullable::Null` disables automatic commits; `Unset` leaves unchanged).
    pub auto_commit_policy: Nullable<JsonObject>,
    /// Event/memory extraction configuration.
    pub memory_extraction_config: Option<JsonObject>,
    /// Optional telemetry configuration.
    pub telemetry: Option<Value>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

/// Options for `commit_session` (`POST /api/v1/sessions/{session_id}/commit`).
#[derive(Debug, Clone, Default)]
pub struct CommitSessionOptions {
    /// Number of recent messages retained in active session history (defaults to 0).
    pub keep_recent_count: i64,
    /// Override working memory extraction (`Some(None)` sends `null` to inherit policy).
    pub enable_working_memory: Option<Option<bool>>,
    /// Turn-based retention mode (`"turn_budget"`).
    pub retention_mode: Option<String>,
    /// Number of recent turns retained.
    pub keep_recent_turn_count: Option<i64>,
    /// Token budget for retained messages.
    pub retained_message_token_budget: Option<i64>,
    /// Minimum raw tail steps retained.
    pub min_raw_tail_steps: Option<i64>,
    /// Event tags applied to extracted event memories (`Some(vec![])` skips session defaults).
    pub event_tags: Option<Vec<String>>,
    /// Optional telemetry configuration.
    pub telemetry: Option<Value>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

/// Options for `batch_add_messages`.
#[derive(Debug, Clone, Default)]
pub struct BatchAddMessagesOptions {
    /// Optional telemetry configuration.
    pub telemetry: Option<Value>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

/// Options for `create_compile_task` and `compile`.
#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    /// Optional compilation instruction prompt.
    pub instruction: Option<String>,
    /// Optional scheduling or model arguments.
    pub args: Option<JsonObject>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

/// Request discriminator for `compile` (`/api/v1/compile`), matching Python validation rules.
#[derive(Debug, Clone)]
pub enum CompileRequest {
    /// Trigger a previously saved persistent compile configuration by `compile_name`.
    Named {
        /// Persistent compile task name.
        compile_name: String,
        /// Optional runtime `args` (only `args` is allowed for named runs).
        options: CompileOptions,
    },
    /// Execute a standalone compilation from explicit inputs.
    Full {
        /// Source URIs (may be empty only when `skill == "memory"`).
        from_uris: Vec<String>,
        /// Output destination URI.
        to: String,
        /// Compilation skill URI or `"memory"`.
        skill: String,
        /// Optional instruction and arguments.
        options: CompileOptions,
    },
}

/// Source entry accepted by `rule_compile` (`/api/v1/rules/compile`).
#[derive(Debug, Clone)]
pub enum RuleCompileSource {
    /// `viking://` resource URI, local file/directory path (uploaded automatically), or remote URL.
    PathOrUri(String),
    /// Structured resource-import or `resource_uri` dictionary.
    Structured(JsonObject),
}

impl From<&str> for RuleCompileSource {
    fn from(s: &str) -> Self {
        Self::PathOrUri(s.to_owned())
    }
}

impl From<String> for RuleCompileSource {
    fn from(s: String) -> Self {
        Self::PathOrUri(s)
    }
}

impl From<&Path> for RuleCompileSource {
    fn from(path: &Path) -> Self {
        Self::PathOrUri(path.to_string_lossy().into_owned())
    }
}

impl From<PathBuf> for RuleCompileSource {
    fn from(path: PathBuf) -> Self {
        Self::PathOrUri(path.to_string_lossy().into_owned())
    }
}

impl From<JsonObject> for RuleCompileSource {
    fn from(map: JsonObject) -> Self {
        Self::Structured(map)
    }
}

/// Options for `rule_compile` (`/api/v1/rules/compile`).
#[derive(Debug, Clone, Default)]
pub struct RuleCompileOptions {
    /// Rule sources (URIs, local paths, or connector descriptors).
    pub sources: Vec<RuleCompileSource>,
}

/// Options for `rule_search` (`/api/v1/rules/search`).
#[derive(Debug, Clone, Default)]
pub struct RuleSearchOptions {
    /// Minimum probability threshold in `[0.0, 1.0]` for applicable rules.
    pub rule_probability_threshold: Option<f64>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

/// Filter options for `list_tasks` (`GET /api/v1/tasks`).
#[derive(Debug, Clone)]
pub struct ListTasksOptions {
    /// Filter by task type.
    pub task_type: Option<String>,
    /// Filter by task status (`"pending"`, `"running"`, `"completed"`, `"failed"`, `"cancelled"`).
    pub status: Option<String>,
    /// Filter by associated resource ID.
    pub resource_id: Option<String>,
    /// Maximum tasks returned (defaults to 50).
    pub limit: usize,
}

impl Default for ListTasksOptions {
    fn default() -> Self {
        Self {
            task_type: None,
            status: None,
            resource_id: None,
            limit: 50,
        }
    }
}

/// Options for `import_ovpack` and `restore_ovpack`.
#[derive(Debug, Clone, Default)]
pub struct ImportPackOptions {
    /// Conflict resolution policy (`"fail"`, `"overwrite"`, `"skip"`).
    pub on_conflict: Option<String>,
    /// Vector handling mode (`"auto"`, `"recompute"`, `"require"`).
    pub vector_mode: Option<String>,
}

/// Target selector for watch mutation/trigger endpoints (`task_id`, `to_uri`, or both).
#[derive(Debug, Clone, Default)]
pub struct WatchRef {
    /// Watch task identifier.
    pub task_id: Option<String>,
    /// Watched destination `viking://` URI.
    pub to_uri: Option<String>,
}

impl WatchRef {
    /// Targets a watch task by `task_id`.
    pub fn by_task_id(task_id: impl Into<String>) -> Self {
        Self {
            task_id: Some(task_id.into()),
            to_uri: None,
        }
    }

    /// Targets a watch task by destination `to_uri`.
    pub fn by_to_uri(to_uri: impl Into<String>) -> Self {
        Self {
            task_id: None,
            to_uri: Some(to_uri.into()),
        }
    }
}

/// Partial update fields for `update_watch`.
#[derive(Debug, Clone, Default)]
pub struct UpdateWatchOptions {
    /// Watch task selector (`task_id` or `to_uri` is required).
    pub target: WatchRef,
    /// Updated watch interval.
    pub watch_interval: Option<f64>,
    /// Pause (`Some(false)`) or resume (`Some(true)`) the watch.
    pub is_active: Option<bool>,
    /// Updated import reason.
    pub reason: Option<String>,
    /// Updated extraction instruction.
    pub instruction: Option<String>,
}

/// Options for `git_commit` (`/api/v1/snapshot/commit`).
#[derive(Debug, Clone)]
pub struct SnapshotCommitOptions {
    /// Commit message.
    pub message: String,
    /// Optional subset of `viking://` paths to commit.
    pub paths: Option<Vec<String>>,
    /// Target branch name (defaults to `"main"`).
    pub branch: String,
    /// Optional commit author name.
    pub author_name: Option<String>,
    /// Optional commit author email.
    pub author_email: Option<String>,
}

impl SnapshotCommitOptions {
    /// Creates commit options on `"main"` with `message`.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            paths: None,
            branch: "main".to_owned(),
            author_name: None,
            author_email: None,
        }
    }
}

/// Options for `git_restore` (`/api/v1/snapshot/restore`).
#[derive(Debug, Clone)]
pub struct SnapshotRestoreOptions {
    /// Source commit SHA or ref to restore from.
    pub source_commit: String,
    /// Optional subtree directory URI (`None` restores the full account tree).
    pub project_dir: Option<String>,
    /// Target branch (defaults to `"main"`).
    pub branch: String,
    /// Preview changes without applying them.
    pub dry_run: bool,
    /// Optional forward-commit message.
    pub message: Option<String>,
    /// Optional commit author name.
    pub author_name: Option<String>,
    /// Optional commit author email.
    pub author_email: Option<String>,
}

impl SnapshotRestoreOptions {
    /// Creates restore options from `source_commit` on `"main"`.
    pub fn new(source_commit: impl Into<String>) -> Self {
        Self {
            source_commit: source_commit.into(),
            project_dir: None,
            branch: "main".to_owned(),
            dry_run: false,
            message: None,
            author_name: None,
            author_email: None,
        }
    }
}

/// Options for `git_log` (`GET /api/v1/snapshot/log`).
#[derive(Debug, Clone)]
pub struct SnapshotLogOptions {
    /// Branch name (defaults to `"main"`).
    pub branch: String,
    /// Maximum commits returned (defaults to 20).
    pub limit: usize,
    /// Number of matching commits skipped (defaults to 0).
    pub offset: usize,
    /// Optional path filter list.
    pub paths: Option<Vec<String>>,
}

impl Default for SnapshotLogOptions {
    fn default() -> Self {
        Self {
            branch: "main".to_owned(),
            limit: 20,
            offset: 0,
            paths: None,
        }
    }
}

/// Options for `git_diff_paths` (`GET /api/v1/snapshot/diff-paths`).
#[derive(Debug, Clone)]
pub struct SnapshotDiffPathsOptions {
    /// Target snapshot ref (`to`).
    pub to_ref: String,
    /// Optional base snapshot ref (`from`).
    pub from_ref: Option<String>,
    /// Pagination offset (defaults to 0).
    pub offset: usize,
    /// Page size limit (defaults to 100).
    pub limit: usize,
}

impl SnapshotDiffPathsOptions {
    /// Creates `git_diff_paths` options comparing against `to_ref`.
    pub fn new(to_ref: impl Into<String>) -> Self {
        Self {
            to_ref: to_ref.into(),
            from_ref: None,
            offset: 0,
            limit: 100,
        }
    }
}

/// Optional fields for `admin_create_account` and `admin_register_user`.
#[derive(Debug, Clone, Default)]
pub struct AdminUserInitOptions {
    /// Initial server-side user configuration object.
    pub user_config: Option<JsonObject>,
    /// Optional deterministic key-derivation seed.
    pub seed: Option<String>,
}

/// Filter and pagination options for `admin_list_accounts`.
#[derive(Debug, Clone)]
pub struct AdminListAccountsOptions {
    /// Wildcard account name filter (`*` and `?`).
    pub name: Option<String>,
    /// Optional page size.
    pub limit: Option<usize>,
    /// 1-based page number when `limit` is set (defaults to 1).
    pub page: usize,
    /// Optional search query filter.
    pub query: Option<String>,
}

impl Default for AdminListAccountsOptions {
    fn default() -> Self {
        Self {
            name: None,
            limit: None,
            page: 1,
            query: None,
        }
    }
}

/// Filter and pagination options for `admin_list_users`.
#[derive(Debug, Clone)]
pub struct AdminListUsersOptions {
    /// Optional page size.
    pub limit: Option<usize>,
    /// Wildcard user ID filter.
    pub name: Option<String>,
    /// Role filter (`"admin"`, `"user"`, etc.).
    pub role: Option<String>,
    /// 1-based page number when `limit` is set (defaults to 1).
    pub page: usize,
    /// Optional search query filter.
    pub query: Option<String>,
}

impl Default for AdminListUsersOptions {
    fn default() -> Self {
        Self {
            limit: None,
            name: None,
            role: None,
            page: 1,
            query: None,
        }
    }
}

/// Pagination and date-range options for `list_experience_trajectories`.
#[derive(Debug, Clone, Default)]
pub struct ExperienceTrajectoryOptions {
    /// Maximum trajectories returned.
    pub limit: Option<usize>,
    /// Pagination offset.
    pub offset: Option<usize>,
    /// Inclusive lower date bound.
    pub start_date: Option<String>,
    /// Inclusive upper date bound.
    pub end_date: Option<String>,
}

/// Date-range options for `get_experience_outcomes`.
#[derive(Debug, Clone, Default)]
pub struct ExperienceOutcomeOptions {
    /// Inclusive lower date bound.
    pub start_date: Option<String>,
    /// Inclusive upper date bound.
    pub end_date: Option<String>,
}

/// Options for `resolve_openviking_assets` (`/api/v1/openviking-assets/resolve`).
#[derive(Debug, Clone, Default)]
pub struct ResolveAssetsOptions {
    /// Optional catalog YAML text.
    pub catalog_yaml: Option<String>,
    /// Optional manifest source label for diagnostics.
    pub manifest_label: Option<String>,
    /// Optional catalog source label for diagnostics.
    pub catalog_label: Option<String>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

/// One-shot Git credentials for `preflight_openviking_asset`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssetGitAuth {
    /// Optional Git username.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Optional personal access token or password.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

/// Options for `preflight_openviking_asset` (`/api/v1/openviking-assets/preflight`).
#[derive(Debug, Clone, Default)]
pub struct PreflightAssetOptions {
    /// Optional branch name.
    pub branch: Option<String>,
    /// Optional commit SHA.
    pub commit: Option<String>,
    /// Optional Git authentication credentials.
    pub auth_config: Option<AssetGitAuth>,
    /// Forward-compatible server fields.
    pub extra: JsonObject,
}

/// Drops `null`-valued keys and empty `args: {}` maps from a request payload, matching Python
/// `_compact_request_body` so older `extra="forbid"` servers accept omitted optional fields.
pub(crate) fn compact_request_body(body: &mut JsonObject) {
    body.retain(|key, value| {
        if value.is_null() {
            return false;
        }
        if key == "args" && value.as_object().is_some_and(Map::is_empty) {
            return false;
        }
        true
    });
}

/// Merges forward-compatible `extra` fields into `body` while rejecting overrides of defined keys.
pub(crate) fn merge_extra(
    body: &mut JsonObject,
    extra: &JsonObject,
    protected: &[&str],
) -> Result<()> {
    for (key, value) in extra {
        if body.contains_key(key) || protected.contains(&key.as_str()) {
            return Err(OpenVikingError::invalid_argument(format!(
                "extra cannot override '{key}'"
            )));
        }
        if !value.is_null() {
            body.insert(key.clone(), value.clone());
        }
    }
    Ok(())
}
