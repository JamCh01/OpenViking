// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Structured session messages and message parts (`TextPart`, `ContextPart`, `ImagePart`, `ToolPart`).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{JsonObject, OpenVikingError, Result};

/// Plain text message part (`{"type": "text", "text": "..."}`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextPart {
    /// Text body of the part.
    pub text: String,
}

impl TextPart {
    /// Creates a new [`TextPart`].
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

/// OpenViking context reference part (`{"type": "context", ...}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPart {
    /// Referenced `viking://` URI.
    pub uri: String,
    /// Context category (`"memory"`, `"resource"`, or `"skill"`).
    pub context_type: String,
    /// Short summary/abstract text.
    #[serde(rename = "abstract")]
    pub abstract_text: String,
}

impl Default for ContextPart {
    fn default() -> Self {
        Self {
            uri: String::new(),
            context_type: "memory".to_owned(),
            abstract_text: String::new(),
        }
    }
}

impl ContextPart {
    /// Creates a new [`ContextPart`].
    pub fn new(
        uri: impl Into<String>,
        context_type: impl Into<String>,
        abstract_text: impl Into<String>,
    ) -> Self {
        Self {
            uri: uri.into(),
            context_type: context_type.into(),
            abstract_text: abstract_text.into(),
        }
    }
}

/// Image URL message part (`{"type": "image_url", "image_url": {"url": "...", ...}}`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImagePart {
    /// Image URL or `data:image/...` URI.
    pub url: String,
    /// Optional detail hint (e.g. `"low"`, `"high"`, `"auto"`).
    pub detail: Option<String>,
}

impl ImagePart {
    /// Creates a new [`ImagePart`].
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            detail: None,
        }
    }

    /// Sets the optional vision `detail` hint.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Tool invocation and output telemetry part (`{"type": "tool", ...}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolPart {
    /// Unique tool call identifier.
    pub tool_id: String,
    /// Invoked tool name.
    pub tool_name: String,
    /// Optional `viking://` URI of the MCP tool descriptor.
    pub tool_uri: String,
    /// Optional `viking://` URI of the associated skill.
    pub skill_uri: String,
    /// Optional JSON arguments supplied to the tool.
    pub tool_input: Option<JsonObject>,
    /// Inline tool output text.
    pub tool_output: String,
    /// Execution status (`"pending"`, `"completed"`, `"failed"`, etc.).
    pub tool_status: String,
    /// Execution duration in milliseconds.
    pub duration_ms: Option<f64>,
    /// Prompt tokens consumed.
    pub prompt_tokens: Option<i64>,
    /// Completion tokens generated.
    pub completion_tokens: Option<i64>,
    /// Externalized output reference URI.
    pub tool_output_ref: String,
    /// Whether `tool_output` was truncated.
    pub tool_output_truncated: bool,
    /// Original character count prior to truncation.
    pub tool_output_original_chars: Option<i64>,
    /// Character count retained in preview.
    pub tool_output_preview_chars: Option<i64>,
    /// SHA-256 digest of the full output.
    pub tool_output_sha256: String,
    /// Backing storage URI for externalized output.
    pub tool_output_storage_uri: String,
    /// MIME type of tool output (defaults to `"text/plain"`).
    pub tool_output_mime_type: String,
    /// Optional source file/URI reference.
    pub tool_output_source_ref: String,
    /// Optional source read offset.
    pub tool_output_source_offset: Option<i64>,
    /// Optional source read limit.
    pub tool_output_source_limit: Option<i64>,
    /// Error string if externalization failed.
    pub tool_output_externalization_error: String,
    /// Output budget group identifier.
    pub tool_output_group_id: String,
    /// Reason why output was externalized.
    pub tool_output_externalized_reason: String,
    /// Original total characters in the group.
    pub tool_output_group_original_chars: Option<i64>,
    /// Character budget assigned to the group.
    pub tool_output_group_budget_chars: Option<i64>,
}

impl Default for ToolPart {
    fn default() -> Self {
        Self {
            tool_id: String::new(),
            tool_name: String::new(),
            tool_uri: String::new(),
            skill_uri: String::new(),
            tool_input: None,
            tool_output: String::new(),
            tool_status: "pending".to_owned(),
            duration_ms: None,
            prompt_tokens: None,
            completion_tokens: None,
            tool_output_ref: String::new(),
            tool_output_truncated: false,
            tool_output_original_chars: None,
            tool_output_preview_chars: None,
            tool_output_sha256: String::new(),
            tool_output_storage_uri: String::new(),
            tool_output_mime_type: "text/plain".to_owned(),
            tool_output_source_ref: String::new(),
            tool_output_source_offset: None,
            tool_output_source_limit: None,
            tool_output_externalization_error: String::new(),
            tool_output_group_id: String::new(),
            tool_output_externalized_reason: String::new(),
            tool_output_group_original_chars: None,
            tool_output_group_budget_chars: None,
        }
    }
}

impl ToolPart {
    /// Creates a new [`ToolPart`] with `tool_id` and `tool_name`.
    pub fn new(tool_id: impl Into<String>, tool_name: impl Into<String>) -> Self {
        Self {
            tool_id: tool_id.into(),
            tool_name: tool_name.into(),
            ..Self::default()
        }
    }
}

/// Union of structured message parts accepted by session message APIs.
#[derive(Debug, Clone, PartialEq)]
pub enum MessagePart {
    /// Text content part.
    Text(TextPart),
    /// Context URI reference part.
    Context(ContextPart),
    /// Image URL part.
    Image(ImagePart),
    /// Tool execution part.
    Tool(Box<ToolPart>),
    /// Pre-serialized wire dictionary part.
    Raw(JsonObject),
}

impl From<TextPart> for MessagePart {
    fn from(part: TextPart) -> Self {
        Self::Text(part)
    }
}

impl From<ContextPart> for MessagePart {
    fn from(part: ContextPart) -> Self {
        Self::Context(part)
    }
}

impl From<ImagePart> for MessagePart {
    fn from(part: ImagePart) -> Self {
        Self::Image(part)
    }
}

impl From<ToolPart> for MessagePart {
    fn from(part: ToolPart) -> Self {
        Self::Tool(Box::new(part))
    }
}

impl From<JsonObject> for MessagePart {
    fn from(raw: JsonObject) -> Self {
        Self::Raw(raw)
    }
}

impl MessagePart {
    /// Normalizes a [`MessagePart`] into the exact HTTP wire JSON object sent by the Python SDK.
    pub fn to_wire_object(&self) -> JsonObject {
        match self {
            Self::Raw(map) => map.clone(),
            Self::Text(part) => {
                let mut map = Map::new();
                map.insert("type".to_owned(), Value::String("text".to_owned()));
                map.insert("text".to_owned(), Value::String(part.text.clone()));
                map
            }
            Self::Context(part) => {
                let mut map = Map::new();
                map.insert("type".to_owned(), Value::String("context".to_owned()));
                map.insert("uri".to_owned(), Value::String(part.uri.clone()));
                map.insert(
                    "context_type".to_owned(),
                    Value::String(part.context_type.clone()),
                );
                map.insert(
                    "abstract".to_owned(),
                    Value::String(part.abstract_text.clone()),
                );
                map
            }
            Self::Image(part) => {
                let mut image_url = Map::new();
                image_url.insert("url".to_owned(), Value::String(part.url.clone()));
                if let Some(detail) = &part.detail {
                    image_url.insert("detail".to_owned(), Value::String(detail.clone()));
                }
                let mut map = Map::new();
                map.insert("type".to_owned(), Value::String("image_url".to_owned()));
                map.insert("image_url".to_owned(), Value::Object(image_url));
                map
            }
            Self::Tool(part) => {
                let mut map = Map::new();
                map.insert("type".to_owned(), Value::String("tool".to_owned()));
                map.insert("tool_id".to_owned(), Value::String(part.tool_id.clone()));
                map.insert(
                    "tool_name".to_owned(),
                    Value::String(part.tool_name.clone()),
                );
                map.insert(
                    "tool_status".to_owned(),
                    Value::String(part.tool_status.clone()),
                );

                insert_if_non_empty(&mut map, "tool_uri", &part.tool_uri);
                insert_if_non_empty(&mut map, "skill_uri", &part.skill_uri);
                if let Some(tool_input) = &part.tool_input {
                    map.insert("tool_input".to_owned(), Value::Object(tool_input.clone()));
                }
                insert_if_non_empty(&mut map, "tool_output", &part.tool_output);
                insert_if_non_empty(&mut map, "tool_output_ref", &part.tool_output_ref);
                insert_if_non_empty(&mut map, "tool_output_sha256", &part.tool_output_sha256);
                insert_if_non_empty(
                    &mut map,
                    "tool_output_storage_uri",
                    &part.tool_output_storage_uri,
                );
                insert_if_non_empty(
                    &mut map,
                    "tool_output_source_ref",
                    &part.tool_output_source_ref,
                );
                insert_if_non_empty(
                    &mut map,
                    "tool_output_externalization_error",
                    &part.tool_output_externalization_error,
                );
                insert_if_non_empty(&mut map, "tool_output_group_id", &part.tool_output_group_id);
                insert_if_non_empty(
                    &mut map,
                    "tool_output_externalized_reason",
                    &part.tool_output_externalized_reason,
                );

                if let Some(duration_ms) = part.duration_ms {
                    if let Some(num) = serde_json::Number::from_f64(duration_ms) {
                        map.insert("duration_ms".to_owned(), Value::Number(num));
                    }
                }
                insert_opt_i64(&mut map, "prompt_tokens", part.prompt_tokens);
                insert_opt_i64(&mut map, "completion_tokens", part.completion_tokens);
                insert_opt_i64(
                    &mut map,
                    "tool_output_original_chars",
                    part.tool_output_original_chars,
                );
                insert_opt_i64(
                    &mut map,
                    "tool_output_preview_chars",
                    part.tool_output_preview_chars,
                );
                insert_opt_i64(
                    &mut map,
                    "tool_output_source_offset",
                    part.tool_output_source_offset,
                );
                insert_opt_i64(
                    &mut map,
                    "tool_output_source_limit",
                    part.tool_output_source_limit,
                );
                insert_opt_i64(
                    &mut map,
                    "tool_output_group_original_chars",
                    part.tool_output_group_original_chars,
                );
                insert_opt_i64(
                    &mut map,
                    "tool_output_group_budget_chars",
                    part.tool_output_group_budget_chars,
                );

                if part.tool_output_truncated {
                    map.insert("tool_output_truncated".to_owned(), Value::Bool(true));
                }
                if !part.tool_output_mime_type.is_empty()
                    && part.tool_output_mime_type != "text/plain"
                {
                    map.insert(
                        "tool_output_mime_type".to_owned(),
                        Value::String(part.tool_output_mime_type.clone()),
                    );
                }
                map
            }
        }
    }
}

/// Structured session message accepted by `add_message` and `batch_add_messages`.
#[derive(Debug, Clone, Default)]
pub struct Message {
    /// Sender role (`"user"`, `"assistant"`, `"system"`, etc.).
    pub role: String,
    /// Optional plain-text content (omitted when `parts` is non-empty).
    pub content: Option<String>,
    /// Optional structured parts.
    pub parts: Vec<MessagePart>,
    /// Optional ISO-8601 creation timestamp.
    pub created_at: Option<String>,
    /// Optional peer identifier associated with this message.
    pub peer_id: Option<String>,
    /// Optional conversation turn identifier.
    pub turn_id: Option<String>,
    /// Optional message classification (`"user_query"`, `"assistant_step"`, `"tool_transport"`, `"checkpoint"`).
    pub message_kind: Option<String>,
    /// Optional upstream message IDs summarized by this message.
    pub source_message_ids: Option<Vec<String>>,
    /// Optional telemetry value.
    pub telemetry: Option<Value>,
    /// Forward-compatible server fields not yet typed by the SDK.
    pub extra: JsonObject,
}

impl Message {
    /// Creates a plain-text message for `role`.
    pub fn text(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: Some(content.into()),
            ..Self::default()
        }
    }

    /// Creates a structured-parts message for `role`.
    pub fn with_parts<I, P>(role: impl Into<String>, parts: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<MessagePart>,
    {
        Self {
            role: role.into(),
            parts: parts.into_iter().map(Into::into).collect(),
            ..Self::default()
        }
    }

    /// Sets `peer_id` on this message.
    pub fn peer_id(mut self, peer_id: impl Into<String>) -> Self {
        self.peer_id = Some(peer_id.into());
        self
    }

    /// Sets `created_at` on this message.
    pub fn created_at(mut self, created_at: impl Into<String>) -> Self {
        self.created_at = Some(created_at.into());
        self
    }

    /// Sets `turn_id` on this message.
    pub fn turn_id(mut self, turn_id: impl Into<String>) -> Self {
        self.turn_id = Some(turn_id.into());
        self
    }

    /// Sets `message_kind` on this message.
    pub fn message_kind(mut self, kind: impl Into<String>) -> Self {
        self.message_kind = Some(kind.into());
        self
    }

    /// Serializes and validates the message into wire JSON, matching Python `_normalize_message_payload`.
    pub fn to_wire_object(&self) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("role".to_owned(), Value::String(self.role.clone()));

        if !self.parts.is_empty() {
            let parts = self
                .parts
                .iter()
                .map(|part| Value::Object(part.to_wire_object()))
                .collect();
            payload.insert("parts".to_owned(), Value::Array(parts));
        } else if let Some(content) = &self.content {
            payload.insert("content".to_owned(), Value::String(content.clone()));
        } else {
            return Err(OpenVikingError::invalid_argument(
                "Either content or non-empty parts must be provided",
            ));
        }

        if let Some(created_at) = &self.created_at {
            payload.insert("created_at".to_owned(), Value::String(created_at.clone()));
        }
        if let Some(peer_id) = &self.peer_id {
            payload.insert("peer_id".to_owned(), Value::String(peer_id.clone()));
        }
        if let Some(turn_id) = &self.turn_id {
            payload.insert("turn_id".to_owned(), Value::String(turn_id.clone()));
        }
        if let Some(message_kind) = &self.message_kind {
            payload.insert(
                "message_kind".to_owned(),
                Value::String(message_kind.clone()),
            );
        }
        if let Some(source_ids) = &self.source_message_ids {
            payload.insert(
                "source_message_ids".to_owned(),
                Value::Array(source_ids.iter().cloned().map(Value::String).collect()),
            );
        }
        if let Some(telemetry) = &self.telemetry {
            if !telemetry.is_null() {
                payload.insert("telemetry".to_owned(), telemetry.clone());
            }
        }
        for (key, value) in &self.extra {
            if payload.contains_key(key) || matches!(key.as_str(), "role" | "content" | "parts") {
                return Err(OpenVikingError::invalid_argument(format!(
                    "extra cannot override '{key}'"
                )));
            }
            if !value.is_null() {
                payload.insert(key.clone(), value.clone());
            }
        }
        Ok(payload)
    }
}

fn insert_if_non_empty(map: &mut JsonObject, key: &str, value: &str) {
    if !value.is_empty() {
        map.insert(key.to_owned(), Value::String(value.to_owned()));
    }
}

fn insert_opt_i64(map: &mut JsonObject, key: &str, value: Option<i64>) {
    if let Some(v) = value {
        map.insert(key.to_owned(), Value::Number(v.into()));
    }
}
