// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Internal pure-async HTTP transport handling identity headers, task-local actor-peer overrides,
//! LDAP/OIDC credentials, Gateway Token 401 challenge/retry, and JSON envelope decoding.

use std::path::Path;
use std::sync::Arc;

use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use reqwest::multipart::{Form, Part};
use serde::de::DeserializeOwned;
use serde_json::Value;
use url::Url;

use crate::actor_peer::get_actor_peer_id;
use crate::config::{get_basic_auth_header, ClientConfig};
use crate::error::{ErrorKind, OpenVikingError, Result};
use crate::types::ResponseEnvelope;

/// Response header set by VikingBot Gateway on 401 token challenges.
pub const GATEWAY_MARKER_HEADER: &str = "X-VikingBot-Gateway";
/// Request header carrying the VikingBot Gateway token.
pub const GATEWAY_TOKEN_HEADER: &str = "X-Gateway-Token";

/// Query parameter list allowing repeated keys (e.g. `tags=a&tags=b`).
pub type QueryPairs = Vec<(String, String)>;

/// Optional request observer hook invoked right before sending each HTTP request.
pub type RequestHook = Arc<dyn Fn(&Method, &Url, &mut HeaderMap) + Send + Sync>;

/// Shared internal HTTP transport cloned cheaply across handles and namespaces.
#[derive(Clone)]
pub(crate) struct Transport {
    base_url: String,
    http: reqwest::Client,
    default_headers: HeaderMap,
    profile_enabled: bool,
    gateway_token: Option<String>,
    upload_mode: Option<String>,
    has_static_gateway_header: bool,
    request_hooks: Vec<RequestHook>,
}

impl std::fmt::Debug for Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transport")
            .field("base_url", &self.base_url)
            .field("profile_enabled", &self.profile_enabled)
            .field("upload_mode", &self.upload_mode)
            .finish_non_exhaustive()
    }
}

impl Transport {
    pub(crate) fn new(config: ClientConfig, request_hooks: Vec<RequestHook>) -> Result<Self> {
        let mut default_headers = HeaderMap::new();

        if let Some(api_key) = &config.api_key {
            insert_header(&mut default_headers, "X-API-Key", api_key)?;
        }
        if let Some(account) = &config.account {
            insert_header(&mut default_headers, "X-OpenViking-Account", account)?;
        }
        if let Some(user) = &config.user {
            insert_header(&mut default_headers, "X-OpenViking-User", user)?;
        }
        if let Some(actor_peer_id) = &config.actor_peer_id {
            insert_header(
                &mut default_headers,
                "X-OpenViking-Actor-Peer",
                actor_peer_id,
            )?;
        }

        if config.auth_mode.as_deref() == Some("ldap") {
            if let (Some(username), Some(password)) = (&config.ldap_username, &config.ldap_password)
            {
                let auth = get_basic_auth_header(username, password);
                insert_header(&mut default_headers, "Authorization", &auth)?;
            }
        } else if config.auth_mode.as_deref() == Some("oidc") {
            let token = config.oidc_token.as_deref().or_else(|| {
                config
                    .api_key
                    .as_deref()
                    .filter(|key| key.matches('.').count() == 2)
            });
            if let Some(jwt) = token {
                insert_header(
                    &mut default_headers,
                    "Authorization",
                    &format!("Bearer {jwt}"),
                )?;
            }
        }

        let mut has_static_gateway_header = false;
        for (name, value) in &config.extra_headers {
            if name.eq_ignore_ascii_case(GATEWAY_TOKEN_HEADER) {
                has_static_gateway_header = true;
            }
            insert_header(&mut default_headers, name, value)?;
        }

        let http = match config.http_client {
            Some(client) => client,
            None => reqwest::Client::builder()
                .timeout(config.timeout)
                .build()
                .map_err(OpenVikingError::from)?,
        };

        Ok(Self {
            base_url: config.url,
            http,
            default_headers,
            profile_enabled: config.profile_enabled,
            gateway_token: config.gateway_token,
            upload_mode: config.upload_mode,
            has_static_gateway_header,
            request_hooks,
        })
    }

    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    fn build_url(&self, path: &str, query: &[(String, String)]) -> Result<Url> {
        let normalized_path = if path.starts_with('/') {
            path.to_owned()
        } else {
            format!("/{path}")
        };
        let mut url =
            Url::parse(&format!("{}{normalized_path}", self.base_url)).map_err(|err| {
                OpenVikingError::invalid_argument(format!("Invalid request path '{path}': {err}"))
            })?;
        {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in query {
                pairs.append_pair(key, value);
            }
            if self.profile_enabled {
                pairs.append_pair("profile", "1");
            }
        }
        if url.query() == Some("") {
            url.set_query(None);
        }
        Ok(url)
    }

    fn build_headers(&self, url: &Url, method: &Method, attach_gateway: bool) -> Result<HeaderMap> {
        let mut headers = self.default_headers.clone();
        if let Some(peer_id) = get_actor_peer_id() {
            insert_header(&mut headers, "X-OpenViking-Actor-Peer", &peer_id)?;
        }
        if attach_gateway {
            if let Some(token) = &self.gateway_token {
                insert_header(&mut headers, GATEWAY_TOKEN_HEADER, token)?;
            }
        }
        for hook in &self.request_hooks {
            hook(method, url, &mut headers);
        }
        Ok(headers)
    }

    fn is_gateway_challenge(response: &reqwest::Response) -> bool {
        response.status() == StatusCode::UNAUTHORIZED
            && response
                .headers()
                .get(GATEWAY_MARKER_HEADER)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v.eq_ignore_ascii_case("true"))
    }

    async fn probe_gateway_token_required(&self) -> Result<bool> {
        let url = self.build_url("/health", &[])?;
        let headers = self.build_headers(&url, &Method::GET, false)?;
        let response = self.http.get(url).headers(headers).send().await?;
        Ok(Self::is_gateway_challenge(&response))
    }

    /// Sends an HTTP request with optional query parameters and JSON body, automatically retrying
    /// once with `X-Gateway-Token` if the upstream gateway responds with a marked 401 challenge.
    pub(crate) async fn send_raw(
        &self,
        method: Method,
        path: &str,
        query: &[(String, String)],
        json_body: Option<&Value>,
    ) -> Result<reqwest::Response> {
        let url = self.build_url(path, query)?;
        let headers = self.build_headers(&url, &method, false)?;
        let has_explicit_gateway = self.has_static_gateway_header
            || headers
                .keys()
                .any(|k| k.as_str().eq_ignore_ascii_case(GATEWAY_TOKEN_HEADER));

        let mut req = self
            .http
            .request(method.clone(), url.clone())
            .headers(headers);
        if let Some(body) = json_body {
            req = req.json(body);
        }
        let response = req.send().await?;

        if !Self::is_gateway_challenge(&response)
            || self.gateway_token.is_none()
            || has_explicit_gateway
        {
            return Ok(response);
        }

        let retry_headers = self.build_headers(&url, &method, true)?;
        let mut retry_req = self.http.request(method, url).headers(retry_headers);
        if let Some(body) = json_body {
            retry_req = retry_req.json(body);
        }
        Ok(retry_req.send().await?)
    }

    /// Sends a request and decodes the full [`ResponseEnvelope<T>`].
    pub(crate) async fn request_envelope<T: DeserializeOwned + Default>(
        &self,
        method: Method,
        path: &str,
        query: &[(String, String)],
        json_body: Option<&Value>,
    ) -> Result<ResponseEnvelope<T>> {
        let response = self.send_raw(method, path, query, json_body).await?;
        Self::parse_envelope(response).await
    }

    /// Sends a request and unwraps `envelope.result` (returning `T::default()` if `result` is null/absent).
    pub(crate) async fn request_result<T: DeserializeOwned + Default>(
        &self,
        method: Method,
        path: &str,
        query: &[(String, String)],
        json_body: Option<&Value>,
    ) -> Result<T> {
        let envelope = self
            .request_envelope::<T>(method, path, query, json_body)
            .await?;
        Ok(envelope.result.unwrap_or_default())
    }

    /// Sends a request whose response body is ignored on success (`2xx` and `status != "error"`).
    pub(crate) async fn request_unit(
        &self,
        method: Method,
        path: &str,
        query: &[(String, String)],
        json_body: Option<&Value>,
    ) -> Result<()> {
        let _ = self
            .request_envelope::<Value>(method, path, query, json_body)
            .await?;
        Ok(())
    }

    /// Uploads a local file to `/api/v1/resources/temp_upload` and returns `temp_file_id`.
    pub(crate) async fn upload_temp_file(&self, file_path: &Path) -> Result<String> {
        let file_bytes = tokio::fs::read(file_path).await?;
        let file_name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "upload".to_owned());

        let attach_gateway = if self.gateway_token.is_some() && !self.has_static_gateway_header {
            self.probe_gateway_token_required().await?
        } else {
            false
        };

        let url = self.build_url("/api/v1/resources/temp_upload", &[])?;
        let headers = self.build_headers(&url, &Method::POST, attach_gateway)?;

        let file_part = Part::bytes(file_bytes)
            .file_name(file_name)
            .mime_str("application/octet-stream")
            .map_err(|err| OpenVikingError::invalid_argument(err.to_string()))?;
        let mut form = Form::new().part("file", file_part);
        if let Some(mode) = &self.upload_mode {
            form = form.text("upload_mode", mode.clone());
        }

        let response = self
            .http
            .post(url)
            .headers(headers)
            .multipart(form)
            .send()
            .await?;
        let envelope: ResponseEnvelope<Value> = Self::parse_envelope(response).await?;
        let temp_id = envelope
            .result
            .as_ref()
            .and_then(|v| v.get("temp_file_id"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        if temp_id.is_empty() {
            return Err(OpenVikingError::new(
                ErrorKind::Internal,
                "INTERNAL",
                "Upload response did not include temp_file_id",
            ));
        }
        Ok(temp_id)
    }

    /// Parses an HTTP response into a typed [`ResponseEnvelope<T>`], mapping error envelopes and
    /// non-2xx HTTP status codes into [`OpenVikingError`].
    pub(crate) async fn parse_envelope<T: DeserializeOwned + Default>(
        response: reqwest::Response,
    ) -> Result<ResponseEnvelope<T>> {
        let status = response.status();
        let status_code = status.as_u16();
        let bytes = response.bytes().await?;

        if bytes.iter().all(u8::is_ascii_whitespace) {
            if !status.is_success() {
                return Err(OpenVikingError::new(
                    ErrorKind::Internal,
                    "INTERNAL",
                    format!("HTTP {status_code}: empty response"),
                )
                .with_status_code(status_code));
            }
            return Ok(ResponseEnvelope {
                status: Some("ok".to_owned()),
                result: Some(T::default()),
                ..ResponseEnvelope::default()
            });
        }

        // First inspect status/error on generic Value so error envelopes never fail T deserialization.
        let raw_value: Value = match serde_json::from_slice(&bytes) {
            Ok(val) => val,
            Err(parse_err) => {
                let text = String::from_utf8_lossy(&bytes).trim().to_owned();
                if !status.is_success() {
                    let message = if text.is_empty() {
                        format!("HTTP {status_code}: empty response")
                    } else {
                        format!("HTTP {status_code}: {text}")
                    };
                    return Err(
                        OpenVikingError::new(ErrorKind::Internal, "INTERNAL", message)
                            .with_status_code(status_code),
                    );
                }
                return Err(OpenVikingError::from(parse_err));
            }
        };

        if let Some(obj) = raw_value.as_object() {
            let is_error_status = obj
                .get("status")
                .and_then(Value::as_str)
                .is_some_and(|s| s == "error");
            if is_error_status || obj.get("error").is_some_and(|e| !e.is_null()) {
                let err_payload = obj
                    .get("error")
                    .cloned()
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();
                return Err(OpenVikingError::from_api_payload(status_code, err_payload));
            }
            if !status.is_success() {
                let detail_msg = obj
                    .get("detail")
                    .map(|d| match d {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .unwrap_or_else(|| format!("HTTP {status_code}"));
                return Err(
                    OpenVikingError::new(ErrorKind::Unknown, "UNKNOWN", detail_msg)
                        .with_status_code(status_code),
                );
            }
        } else if !status.is_success() {
            return Err(OpenVikingError::new(
                ErrorKind::Unknown,
                "UNKNOWN",
                format!("HTTP {status_code}: {raw_value}"),
            )
            .with_status_code(status_code));
        }

        let envelope: ResponseEnvelope<T> = serde_json::from_value(raw_value)?;
        Ok(envelope)
    }
}

/// Percent-encodes a single URL path segment (matching Python `urllib.parse.quote(value, safe="")`).
pub(crate) fn encode_path_segment(segment: &str) -> String {
    let mut encoded = String::with_capacity(segment.len());
    for byte in segment.as_bytes() {
        let is_unreserved =
            byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'.' | b'_' | b'~');
        if is_unreserved {
            encoded.push(char::from(*byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn insert_header(headers: &mut HeaderMap, name: &str, value: &str) -> Result<()> {
    let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|err| {
        OpenVikingError::config(format!("Invalid HTTP header name '{name}': {err}"))
    })?;
    let header_value = HeaderValue::from_str(value).map_err(|err| {
        OpenVikingError::config(format!("Invalid HTTP header value for '{name}': {err}"))
    })?;
    headers.insert(header_name, header_value);
    Ok(())
}
