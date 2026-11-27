// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Structured error types and server error-code mapping for the OpenViking SDK.

use std::error::Error as StdError;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Convenience alias for SDK results.
pub type Result<T> = std::result::Result<T, OpenVikingError>;

/// JSON object alias used for structured metadata and error details.
pub type JsonObject = Map<String, Value>;

/// Categorized error kinds aligned with the Python `openviking-sdk` exception hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// Server returned `INVALID_ARGUMENT` or caller passed invalid parameters.
    InvalidArgument,
    /// Server returned `INVALID_URI` or URI validation failed.
    InvalidUri,
    /// Server returned `NOT_FOUND` (HTTP 404).
    NotFound,
    /// Server returned `ALREADY_EXISTS` (HTTP 409).
    AlreadyExists,
    /// Server returned `CONFLICT` (HTTP 409).
    Conflict,
    /// Server returned `FAILED_PRECONDITION` (HTTP 412) or precondition check failed.
    FailedPrecondition,
    /// Server returned `ABORTED` or request was cancelled.
    Aborted,
    /// Server returned `UNAUTHENTICATED` (HTTP 401).
    Unauthenticated,
    /// Server returned `PERMISSION_DENIED` (HTTP 403).
    PermissionDenied,
    /// Server returned `RESOURCE_EXHAUSTED` (HTTP 429).
    ResourceExhausted,
    /// Server returned `UNAVAILABLE` (HTTP 503) or service was unreachable.
    Unavailable,
    /// Server returned `INTERNAL` (HTTP 500).
    Internal,
    /// Server returned `DEADLINE_EXCEEDED` (HTTP 504) or request timed out.
    DeadlineExceeded,
    /// Server returned `UNIMPLEMENTED` (HTTP 501).
    Unimplemented,
    /// Server returned `NOT_INITIALIZED`.
    NotInitialized,
    /// Server returned `PROCESSING_ERROR`.
    ProcessingError,
    /// Server returned `EMBEDDING_FAILED`.
    EmbeddingFailed,
    /// Server returned `VLM_FAILED`.
    VlmFailed,
    /// Server returned `SESSION_EXPIRED` (HTTP 410).
    SessionExpired,
    /// Unrecognized or generic server error code (`UNKNOWN` or provider-specific).
    Unknown,
    /// Local client configuration error (`ovcli.conf`, environment variables, or builder options).
    Config,
    /// HTTP/network transport error before a valid response was received.
    Transport,
    /// Local filesystem I/O error (e.g. reading upload sources or writing `.ovpack` files).
    Io,
    /// JSON serialization or deserialization error.
    Serialization,
}

impl ErrorKind {
    /// Maps an OpenViking wire error code string to its corresponding [`ErrorKind`].
    pub fn from_wire_code(code: &str) -> Self {
        match code {
            "INVALID_ARGUMENT" => Self::InvalidArgument,
            "INVALID_URI" => Self::InvalidUri,
            "NOT_FOUND" => Self::NotFound,
            "ALREADY_EXISTS" => Self::AlreadyExists,
            "CONFLICT" => Self::Conflict,
            "FAILED_PRECONDITION" => Self::FailedPrecondition,
            "ABORTED" => Self::Aborted,
            "UNAUTHENTICATED" => Self::Unauthenticated,
            "PERMISSION_DENIED" => Self::PermissionDenied,
            "RESOURCE_EXHAUSTED" => Self::ResourceExhausted,
            "UNAVAILABLE" => Self::Unavailable,
            "INTERNAL" => Self::Internal,
            "DEADLINE_EXCEEDED" => Self::DeadlineExceeded,
            "UNIMPLEMENTED" => Self::Unimplemented,
            "NOT_INITIALIZED" => Self::NotInitialized,
            "PROCESSING_ERROR" => Self::ProcessingError,
            "EMBEDDING_FAILED" => Self::EmbeddingFailed,
            "VLM_FAILED" => Self::VlmFailed,
            "SESSION_EXPIRED" => Self::SessionExpired,
            _ => Self::Unknown,
        }
    }

    /// Returns whether this error represents a processing failure (`PROCESSING_ERROR`,
    /// `EMBEDDING_FAILED`, or `VLM_FAILED`), mirroring Python's `ProcessingError` base class.
    pub fn is_processing_error(self) -> bool {
        matches!(
            self,
            Self::ProcessingError | Self::EmbeddingFailed | Self::VlmFailed
        )
    }

    /// Returns whether this error represents an invalid argument (`INVALID_ARGUMENT` or
    /// `INVALID_URI`), mirroring Python's `InvalidArgumentError` base class.
    pub fn is_invalid_argument(self) -> bool {
        matches!(self, Self::InvalidArgument | Self::InvalidUri)
    }
}

/// Wire format of the `error` object inside an OpenViking HTTP response envelope.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiErrorPayload {
    /// Machine-readable error code (e.g. `"NOT_FOUND"`).
    #[serde(default)]
    pub code: Option<String>,
    /// Human-readable error description.
    #[serde(default)]
    pub message: Option<String>,
    /// Optional structured error context.
    #[serde(default)]
    pub details: Option<JsonObject>,
}

/// Unified error type returned by all fallible operations in `openviking-sdk`.
#[derive(Debug)]
pub struct OpenVikingError {
    kind: ErrorKind,
    code: String,
    message: String,
    details: JsonObject,
    status_code: Option<u16>,
    source: Option<Box<dyn StdError + Send + Sync>>,
}

impl OpenVikingError {
    /// Constructs an error with explicit metadata.
    pub fn new(kind: ErrorKind, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind,
            code: code.into(),
            message: message.into(),
            details: JsonObject::new(),
            status_code: None,
            source: None,
        }
    }

    /// Attaches structured JSON details to this error.
    pub fn with_details(mut self, details: JsonObject) -> Self {
        self.details = details;
        self
    }

    /// Attaches an HTTP status code to this error.
    pub fn with_status_code(mut self, status_code: u16) -> Self {
        self.status_code = Some(status_code);
        self
    }

    /// Attaches an underlying cause to this error.
    pub fn with_source<E>(mut self, source: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        self.source = Some(Box::new(source));
        self
    }

    /// Constructs a local configuration error.
    pub fn config(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Config, "INVALID_CONFIG", message)
    }

    /// Constructs a client-side argument validation error.
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidArgument, "INVALID_ARGUMENT", message)
    }

    /// Constructs a client-side precondition failure error with optional details.
    pub fn failed_precondition(message: impl Into<String>, details: JsonObject) -> Self {
        Self::new(
            ErrorKind::FailedPrecondition,
            "FAILED_PRECONDITION",
            message,
        )
        .with_details(details)
    }

    /// Constructs an error from a server `ApiErrorPayload` and HTTP status code,
    /// matching Python SDK message formatting and detail preservation.
    pub fn from_api_payload(status_code: u16, payload: ApiErrorPayload) -> Self {
        let code = payload
            .code
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| "UNKNOWN".to_owned());
        let raw_message = payload
            .message
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| "Unknown error".to_owned());
        let mut details = payload.details.unwrap_or_default();
        let kind = ErrorKind::from_wire_code(&code);

        let formatted_message = match kind {
            ErrorKind::InvalidUri => {
                let uri = detail_str(&details, "uri").unwrap_or("").to_owned();
                let reason = detail_str(&details, "reason").unwrap_or("").to_owned();
                let mut msg = format!("Invalid URI: {uri}");
                if !reason.is_empty() {
                    msg.push_str(&format!(" ({reason})"));
                }
                details
                    .entry("uri".to_owned())
                    .or_insert(Value::String(uri));
                details
                    .entry("reason".to_owned())
                    .or_insert(Value::String(reason));
                msg
            }
            ErrorKind::NotFound => {
                let resource = detail_str(&details, "resource").unwrap_or("").to_owned();
                let resource_type = detail_str(&details, "type")
                    .unwrap_or("resource")
                    .to_owned();
                let reason = detail_str(&details, "reason").map(str::to_owned);
                let mut msg = if resource.is_empty() {
                    if !raw_message.is_empty()
                        && raw_message != "Unknown error"
                        && details.is_empty()
                    {
                        raw_message
                    } else {
                        format!("{} not found", capitalize(&resource_type))
                    }
                } else {
                    format!("{} not found: {resource}", capitalize(&resource_type))
                };
                if let Some(reason_text) = &reason {
                    if !reason_text.is_empty() {
                        msg.push_str(&format!(". {reason_text}"));
                    }
                }
                details
                    .entry("type".to_owned())
                    .or_insert_with(|| Value::String(resource_type));
                if !resource.is_empty() {
                    details
                        .entry("resource".to_owned())
                        .or_insert_with(|| Value::String(resource));
                }
                msg
            }
            ErrorKind::AlreadyExists => {
                let resource = detail_str(&details, "resource").unwrap_or("").to_owned();
                let resource_type = detail_str(&details, "type")
                    .unwrap_or("resource")
                    .to_owned();
                let msg = format!("{} already exists: {resource}", capitalize(&resource_type));
                details
                    .entry("resource".to_owned())
                    .or_insert_with(|| Value::String(resource));
                details
                    .entry("type".to_owned())
                    .or_insert_with(|| Value::String(resource_type));
                msg
            }
            ErrorKind::Unavailable => {
                if details.is_empty() {
                    format!("Service unavailable: {raw_message}")
                } else {
                    let service = detail_str(&details, "service")
                        .unwrap_or("service")
                        .to_owned();
                    let reason = detail_str(&details, "reason")
                        .unwrap_or(&raw_message)
                        .to_owned();
                    let mut msg = format!("{} unavailable", capitalize(&service));
                    if !reason.is_empty() {
                        msg.push_str(&format!(": {reason}"));
                    }
                    details
                        .entry("service".to_owned())
                        .or_insert_with(|| Value::String(service));
                    details
                        .entry("reason".to_owned())
                        .or_insert_with(|| Value::String(reason));
                    msg
                }
            }
            _ => raw_message,
        };

        Self {
            kind,
            code,
            message: formatted_message,
            details,
            status_code: Some(status_code),
            source: None,
        }
    }

    /// Returns the categorized [`ErrorKind`].
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the server or SDK error code string (e.g. `"NOT_FOUND"`).
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the human-readable error message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns structured error details attached by the server or SDK.
    pub fn details(&self) -> &JsonObject {
        &self.details
    }

    /// Returns the HTTP status code when the error originated from an HTTP response.
    pub fn status_code(&self) -> Option<u16> {
        self.status_code
    }

    /// Returns `true` when `self.code()` matches `expected_code`.
    pub fn is_code(&self, expected_code: &str) -> bool {
        self.code == expected_code
    }

    /// Returns `true` when this error represents a missing resource (`NOT_FOUND` or HTTP 404).
    pub fn is_not_found(&self) -> bool {
        self.kind == ErrorKind::NotFound || self.status_code == Some(404)
    }
}

impl fmt::Display for OpenVikingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl StdError for OpenVikingError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_deref()
            .map(|err| err as &(dyn StdError + 'static))
    }
}

impl From<std::io::Error> for OpenVikingError {
    fn from(err: std::io::Error) -> Self {
        let message = err.to_string();
        Self::new(ErrorKind::Io, "IO_ERROR", message).with_source(err)
    }
}

impl From<serde_json::Error> for OpenVikingError {
    fn from(err: serde_json::Error) -> Self {
        let message = err.to_string();
        Self::new(ErrorKind::Serialization, "SERIALIZATION_ERROR", message).with_source(err)
    }
}

impl From<reqwest::Error> for OpenVikingError {
    fn from(err: reqwest::Error) -> Self {
        let kind = if err.is_timeout() {
            ErrorKind::DeadlineExceeded
        } else {
            ErrorKind::Transport
        };
        let code = if err.is_timeout() {
            "DEADLINE_EXCEEDED"
        } else {
            "TRANSPORT_ERROR"
        };
        let status_code = err.status().map(|s| s.as_u16());
        let mut out = Self::new(kind, code, err.to_string()).with_source(err);
        if let Some(status) = status_code {
            out = out.with_status_code(status);
        }
        out
    }
}

fn detail_str<'a>(details: &'a JsonObject, key: &str) -> Option<&'a str> {
    details.get(key).and_then(Value::as_str)
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = first.to_uppercase().collect::<String>();
    out.push_str(&chars.as_str().to_lowercase());
    out
}
