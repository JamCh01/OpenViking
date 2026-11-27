// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Client configuration and 3-tier resolution (`explicit > env > ovcli.conf`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use serde_json::{Map, Value};

use crate::error::{OpenVikingError, Result};

/// Environment variable pointing to a custom `ovcli.conf` JSON file.
pub const OPENVIKING_CLI_CONFIG_ENV: &str = "OPENVIKING_CLI_CONFIG_FILE";

const DEFAULT_TIMEOUT_SECS: f64 = 60.0;

const ALLOWED_OVCLI_KEYS: &[&str] = &[
    "url",
    "api_key",
    "account",
    "user",
    "actor_peer_id",
    "agent_id",
    "timeout",
    "profile",
    "upload",
    "extra_headers",
    "extra_header",
    "output",
    "echo_command",
    "show_progress",
    "verbose",
    "root_api_key",
    "gateway_token",
    "plugin",
    "auth_mode",
    "ldap_username",
    "ldap_password",
    "oidc_token",
];

const ALLOWED_UPLOAD_KEYS: &[&str] = &["mode", "ignore_dirs", "include", "exclude"];

/// Temporary upload storage mode supported by OpenViking servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadMode {
    /// Local temporary upload directory on a single server instance.
    Local,
    /// Shared temporary upload storage across clustered instances.
    Shared,
}

impl UploadMode {
    /// Parses an upload mode string.
    pub fn from_wire(value: &str) -> Self {
        match value {
            "shared" => Self::Shared,
            _ => Self::Local,
        }
    }

    /// Returns the wire representation sent to `/api/v1/resources/temp_upload`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Shared => "shared",
        }
    }
}

/// Resolved immutable configuration for an [`crate::AsyncHttpClient`].
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Normalized server base URL without trailing slashes.
    pub url: String,
    /// Optional root or user API key (`X-API-Key`).
    pub api_key: Option<String>,
    /// Optional tenant account header (`X-OpenViking-Account`).
    pub account: Option<String>,
    /// Optional tenant user header (`X-OpenViking-User`).
    pub user: Option<String>,
    /// Optional default actor peer header (`X-OpenViking-Actor-Peer`).
    pub actor_peer_id: Option<String>,
    /// Request timeout duration (defaults to 60 seconds).
    pub timeout: Duration,
    /// Whether `?profile=1` is appended to outgoing requests.
    pub profile_enabled: bool,
    /// Additional static HTTP headers attached to every request.
    pub extra_headers: BTreeMap<String, String>,
    /// Optional gateway token attached on `X-VikingBot-Gateway: true` 401 challenges.
    pub gateway_token: Option<String>,
    /// Optional upload mode string (`"local"` or `"shared"`).
    pub upload_mode: Option<String>,
    /// Optional authentication mode (`"ldap"`, `"oidc"`, or `"api_key"`).
    pub auth_mode: Option<String>,
    /// Optional LDAP username when `auth_mode` is `"ldap"`.
    pub ldap_username: Option<String>,
    /// Optional LDAP password when `auth_mode` is `"ldap"`.
    pub ldap_password: Option<String>,
    /// Optional OIDC bearer token when `auth_mode` is `"oidc"`.
    pub oidc_token: Option<String>,
    /// Optional pre-configured `reqwest::Client` override for custom TLS/proxy/test setups.
    pub http_client: Option<reqwest::Client>,
}

impl ClientConfig {
    /// Starts a [`ClientConfigBuilder`] that resolves missing fields from environment variables
    /// and `~/.openviking/ovcli.conf`.
    pub fn builder() -> ClientConfigBuilder {
        ClientConfigBuilder::default()
    }

    /// Resolves configuration purely from environment variables and `ovcli.conf`.
    pub fn from_env() -> Result<Self> {
        Self::builder().build()
    }

    /// Constructs a configuration with an explicit base URL while still allowing remaining fields
    /// to resolve from environment variables and `ovcli.conf`.
    pub fn with_url(url: impl Into<String>) -> Result<Self> {
        Self::builder().url(url).build()
    }
}

/// Parsed representation of `~/.openviking/ovcli.conf`.
#[derive(Debug, Clone, PartialEq)]
pub struct OvCliConfig {
    /// Server base URL.
    pub url: Option<String>,
    /// API key.
    pub api_key: Option<String>,
    /// Tenant account name.
    pub account: Option<String>,
    /// Tenant user name.
    pub user: Option<String>,
    /// Actor peer identifier.
    pub actor_peer_id: Option<String>,
    /// Legacy `agent_id` field from older CLI configs.
    pub agent_id: Option<String>,
    /// Timeout in seconds (defaults to `60.0` when omitted in JSON).
    pub timeout: f64,
    /// Whether server profiling is enabled.
    pub profile: bool,
    /// Static extra HTTP headers.
    pub extra_headers: BTreeMap<String, String>,
    /// Optional gateway token.
    pub gateway_token: Option<String>,
    /// Optional upload mode (`upload.mode`).
    pub upload_mode: Option<String>,
    /// Optional CLI output format.
    pub output: Option<String>,
    /// Optional authentication mode (`ldap`, `oidc`, `api_key`).
    pub auth_mode: Option<String>,
    /// Optional LDAP username.
    pub ldap_username: Option<String>,
    /// Optional LDAP password.
    pub ldap_password: Option<String>,
    /// Optional OIDC token.
    pub oidc_token: Option<String>,
}

/// Environment lookup seam so tests can exercise config precedence without mutating process env.
pub trait EnvSource {
    /// Looks up an environment variable by name.
    fn get_var(&self, key: &str) -> Option<String>;
    /// Returns the user's home directory, if determinable.
    fn home_dir(&self) -> Option<PathBuf>;
}

/// Default [`EnvSource`] backed by `std::env`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemEnv;

impl EnvSource for SystemEnv {
    fn get_var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    fn home_dir(&self) -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }
}

/// Builder for [`ClientConfig`] supporting explicit values, environment variables, and `ovcli.conf`.
#[derive(Debug, Clone, Default)]
pub struct ClientConfigBuilder {
    url: Option<String>,
    api_key: Option<String>,
    account: Option<String>,
    user: Option<String>,
    user_id_legacy: Option<String>,
    actor_peer_id: Option<String>,
    agent_id_legacy: Option<String>,
    timeout_secs: Option<f64>,
    extra_headers: Option<BTreeMap<String, String>>,
    profile_enabled: Option<bool>,
    upload_mode: Option<String>,
    gateway_token: Option<String>,
    auth_mode: Option<String>,
    ldap_username: Option<String>,
    ldap_password: Option<String>,
    oidc_token: Option<String>,
    config_path: Option<PathBuf>,
    ignore_config_file: bool,
    http_client: Option<reqwest::Client>,
}

impl ClientConfigBuilder {
    /// Sets the OpenViking server base URL.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Sets the API key (`X-API-Key`).
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Sets the account header (`X-OpenViking-Account`).
    pub fn account(mut self, account: impl Into<String>) -> Self {
        self.account = Some(account.into());
        self
    }

    /// Sets the user header (`X-OpenViking-User`).
    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Sets the legacy `user_id` alias (used when `user` is not set).
    pub fn user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id_legacy = Some(user_id.into());
        self
    }

    /// Sets the default actor peer header (`X-OpenViking-Actor-Peer`).
    pub fn actor_peer_id(mut self, actor_peer_id: impl Into<String>) -> Self {
        self.actor_peer_id = Some(actor_peer_id.into());
        self
    }

    /// Sets the legacy `agent_id` alias (cannot be combined with `actor_peer_id`).
    pub fn agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id_legacy = Some(agent_id.into());
        self
    }

    /// Sets the request timeout duration.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout_secs = Some(timeout.as_secs_f64());
        self
    }

    /// Sets the request timeout in seconds.
    pub fn timeout_secs(mut self, seconds: f64) -> Self {
        self.timeout_secs = Some(seconds);
        self
    }

    /// Sets static extra HTTP headers (overrides headers from `ovcli.conf`).
    pub fn extra_headers<I, K, V>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.extra_headers = Some(
            headers
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        );
        self
    }

    /// Enables or disables `?profile=1` query attachment.
    pub fn profile_enabled(mut self, enabled: bool) -> Self {
        self.profile_enabled = Some(enabled);
        self
    }

    /// Sets the temporary upload mode (`"local"` or `"shared"`).
    pub fn upload_mode(mut self, mode: impl Into<String>) -> Self {
        self.upload_mode = Some(mode.into());
        self
    }

    /// Sets an explicit gateway token for `X-VikingBot-Gateway` challenges.
    pub fn gateway_token(mut self, token: impl Into<String>) -> Self {
        self.gateway_token = Some(token.into());
        self
    }

    /// Sets the authentication mode (`"ldap"`, `"oidc"`, or `"api_key"`).
    pub fn auth_mode(mut self, mode: impl Into<String>) -> Self {
        self.auth_mode = Some(mode.into());
        self
    }

    /// Sets the LDAP username.
    pub fn ldap_username(mut self, username: impl Into<String>) -> Self {
        self.ldap_username = Some(username.into());
        self
    }

    /// Sets the LDAP password.
    pub fn ldap_password(mut self, password: impl Into<String>) -> Self {
        self.ldap_password = Some(password.into());
        self
    }

    /// Sets the OIDC bearer token.
    pub fn oidc_token(mut self, token: impl Into<String>) -> Self {
        self.oidc_token = Some(token.into());
        self
    }

    /// Overrides the path to `ovcli.conf` instead of consulting `OPENVIKING_CLI_CONFIG_FILE`
    /// or `~/.openviking/ovcli.conf`.
    pub fn config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(path.into());
        self
    }

    /// Disables reading any `ovcli.conf` file from disk.
    pub fn without_config_file(mut self) -> Self {
        self.ignore_config_file = true;
        self
    }

    /// Supplies a custom `reqwest::Client` instance.
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = Some(client);
        self
    }

    /// Resolves and validates the [`ClientConfig`] using the process environment.
    pub fn build(self) -> Result<ClientConfig> {
        self.build_with_env(&SystemEnv)
    }

    /// Resolves and validates the [`ClientConfig`] using a custom [`EnvSource`].
    pub fn build_with_env(self, env: &dyn EnvSource) -> Result<ClientConfig> {
        if self.actor_peer_id.as_deref().is_some_and(|s| !s.is_empty())
            && self
                .agent_id_legacy
                .as_deref()
                .is_some_and(|s| !s.is_empty())
        {
            return Err(OpenVikingError::config(
                "actor_peer_id cannot be used with agent_id",
            ));
        }

        let effective_user = self.user.or(self.user_id_legacy);
        let effective_actor = self.actor_peer_id.or(self.agent_id_legacy);

        let cli_config = if self.ignore_config_file {
            None
        } else if let Some(explicit_path) = &self.config_path {
            load_ovcli_config(Some(explicit_path))?
        } else {
            let resolved_path = resolve_ovcli_config_path(env);
            match resolved_path {
                Some(path) => load_ovcli_config(Some(&path))?,
                None => None,
            }
        };

        let resolved_auth_mode = resolve_str_precedence(
            env,
            "OPENVIKING_AUTH_MODE",
            self.auth_mode,
            cli_config.as_ref().and_then(|c| c.auth_mode.clone()),
        );
        let resolved_ldap_username = resolve_str_precedence(
            env,
            "OPENVIKING_USERNAME",
            self.ldap_username,
            cli_config.as_ref().and_then(|c| c.ldap_username.clone()),
        );
        let resolved_ldap_password = resolve_str_precedence(
            env,
            "OPENVIKING_PASSWORD",
            self.ldap_password,
            cli_config.as_ref().and_then(|c| c.ldap_password.clone()),
        );
        let resolved_oidc_token = resolve_str_precedence(
            env,
            "OPENVIKING_OIDC_TOKEN",
            self.oidc_token,
            cli_config.as_ref().and_then(|c| c.oidc_token.clone()),
        );

        let resolved_url = non_empty(self.url)
            .or_else(|| non_empty(env.get_var("OPENVIKING_URL")))
            .or_else(|| cli_config.as_ref().and_then(|c| non_empty(c.url.clone())));

        let Some(raw_url) = resolved_url else {
            return Err(OpenVikingError::config(
                "url is required. Pass it explicitly, set OPENVIKING_URL, or configure ovcli.conf.",
            ));
        };
        let normalized_url = raw_url.trim_end_matches('/').to_owned();
        let parsed_url = url::Url::parse(&normalized_url).map_err(|err| {
            OpenVikingError::config(format!("Invalid base URL '{normalized_url}': {err}"))
        })?;
        if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
            return Err(OpenVikingError::config(format!(
                "Base URL must use http or https scheme: '{normalized_url}'"
            )));
        }

        let resolved_api_key = non_empty(self.api_key)
            .or_else(|| non_empty(env.get_var("OPENVIKING_API_KEY")))
            .or_else(|| {
                cli_config
                    .as_ref()
                    .and_then(|c| non_empty(c.api_key.clone()))
            });
        let resolved_account = non_empty(self.account)
            .or_else(|| non_empty(env.get_var("OPENVIKING_ACCOUNT")))
            .or_else(|| {
                cli_config
                    .as_ref()
                    .and_then(|c| non_empty(c.account.clone()))
            });
        let resolved_user = non_empty(effective_user)
            .or_else(|| non_empty(env.get_var("OPENVIKING_USER")))
            .or_else(|| cli_config.as_ref().and_then(|c| non_empty(c.user.clone())));
        let mut resolved_actor_peer_id = non_empty(effective_actor)
            .or_else(|| non_empty(env.get_var("OPENVIKING_ACTOR_PEER_ID")))
            .or_else(|| {
                cli_config
                    .as_ref()
                    .and_then(|c| non_empty(c.actor_peer_id.clone()))
            });
        if resolved_actor_peer_id.is_none() {
            if let Some(agent_id) = cli_config
                .as_ref()
                .and_then(|c| non_empty(c.agent_id.clone()))
            {
                resolved_actor_peer_id = Some(agent_id);
            }
        }

        let resolved_timeout_secs = if let Some(explicit_timeout) = self.timeout_secs {
            explicit_timeout
        } else if let Some(env_timeout) = non_empty(env.get_var("OPENVIKING_TIMEOUT")) {
            env_timeout.parse::<f64>().map_err(|_| {
                OpenVikingError::config(format!(
                    "Invalid OPENVIKING_TIMEOUT '{env_timeout}': expected number"
                ))
            })?
        } else if let Some(cfg) = &cli_config {
            cfg.timeout
        } else {
            DEFAULT_TIMEOUT_SECS
        };
        if !resolved_timeout_secs.is_finite() || resolved_timeout_secs <= 0.0 {
            return Err(OpenVikingError::config(
                "timeout must be a positive finite number",
            ));
        }

        let resolved_profile_enabled = match self.profile_enabled {
            Some(enabled) => enabled,
            None => cli_config.as_ref().is_some_and(|c| c.profile),
        };

        let resolved_extra_headers = match self.extra_headers {
            Some(headers) => headers,
            None => cli_config
                .as_ref()
                .map(|c| c.extra_headers.clone())
                .unwrap_or_default(),
        };

        let resolved_gateway_token = if self.gateway_token.is_some() {
            self.gateway_token
        } else if let Some(cfg) = &cli_config {
            if cfg
                .url
                .as_deref()
                .is_some_and(|cfg_url| cfg_url.trim_end_matches('/') == normalized_url)
            {
                cfg.gateway_token.clone()
            } else {
                None
            }
        } else {
            None
        };

        let resolved_upload_mode = self
            .upload_mode
            .or_else(|| cli_config.and_then(|c| c.upload_mode));

        Ok(ClientConfig {
            url: normalized_url,
            api_key: resolved_api_key,
            account: resolved_account,
            user: resolved_user,
            actor_peer_id: resolved_actor_peer_id,
            timeout: Duration::from_secs_f64(resolved_timeout_secs),
            profile_enabled: resolved_profile_enabled,
            extra_headers: resolved_extra_headers,
            gateway_token: resolved_gateway_token,
            upload_mode: resolved_upload_mode,
            auth_mode: resolved_auth_mode,
            ldap_username: resolved_ldap_username,
            ldap_password: resolved_ldap_password,
            oidc_token: resolved_oidc_token,
            http_client: self.http_client,
        })
    }
}

/// Loads and validates an `ovcli.conf` JSON file if it exists.
pub fn load_ovcli_config(config_path: Option<&Path>) -> Result<Option<OvCliConfig>> {
    let resolved;
    let target = match config_path {
        Some(path) => path,
        None => {
            let Some(default_path) = resolve_ovcli_config_path(&SystemEnv) else {
                return Ok(None);
            };
            resolved = default_path;
            &resolved
        }
    };

    if !target.exists() {
        return Ok(None);
    }

    let raw_text = std::fs::read_to_string(target).map_err(|err| {
        OpenVikingError::config(format!(
            "Failed to read CLI config in {}: {err}",
            target.display()
        ))
    })?;
    let raw_value: Value = serde_json::from_str(&raw_text).map_err(|err| {
        OpenVikingError::config(format!("Invalid CLI config in {}: {err}", target.display()))
    })?;

    parse_ovcli_value(&raw_value).map(Some).map_err(|err| {
        OpenVikingError::config(format!(
            "Invalid CLI config in {}: {}",
            target.display(),
            err.message()
        ))
    })
}

/// Encodes `username:password` into an HTTP `Basic <base64>` header value.
pub fn get_basic_auth_header(username: &str, password: &str) -> String {
    let credentials = format!("{username}:{password}");
    format!("Basic {}", BASE64_STANDARD.encode(credentials.as_bytes()))
}

fn resolve_ovcli_config_path(env: &dyn EnvSource) -> Option<PathBuf> {
    if let Some(custom) = non_empty(env.get_var(OPENVIKING_CLI_CONFIG_ENV)) {
        return Some(expand_tilde(&custom, env));
    }
    let home = env.home_dir()?;
    let candidate = home.join(".openviking").join("ovcli.conf");
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

fn expand_tilde(raw: &str, env: &dyn EnvSource) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = env.home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(raw)
}

fn resolve_str_precedence(
    env: &dyn EnvSource,
    env_var: &str,
    explicit: Option<String>,
    config_value: Option<String>,
) -> Option<String> {
    if explicit.is_some() {
        return explicit;
    }
    if let Some(env_val) = env.get_var(env_var) {
        return Some(env_val);
    }
    config_value
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty())
}

fn parse_ovcli_value(raw: &Value) -> Result<OvCliConfig> {
    let data = require_mapping(raw, "ovcli")?;
    check_unknown_keys("ovcli", data, ALLOWED_OVCLI_KEYS)?;

    let extra_headers_val = data
        .get("extra_headers")
        .or_else(|| data.get("extra_header"));
    let extra_headers = parse_extra_headers(extra_headers_val, "ovcli.extra_headers")?;

    let upload_mode = if let Some(upload_val) = data.get("upload") {
        if upload_val.is_null() {
            None
        } else {
            let upload_map = require_mapping(upload_val, "ovcli.upload")?;
            check_unknown_keys("ovcli.upload", upload_map, ALLOWED_UPLOAD_KEYS)?;
            optional_string(upload_map.get("mode"), "ovcli.upload.mode")?
        }
    } else {
        None
    };

    let actor_peer_id = optional_string(data.get("actor_peer_id"), "ovcli.actor_peer_id")?;
    let agent_id = optional_string(data.get("agent_id"), "ovcli.agent_id")?;
    if actor_peer_id.is_some() && agent_id.is_some() {
        return Err(OpenVikingError::config(
            "actor_peer_id cannot be used with agent_id",
        ));
    }

    let timeout =
        optional_float(data.get("timeout"), "ovcli.timeout")?.unwrap_or(DEFAULT_TIMEOUT_SECS);
    let profile = optional_bool(data.get("profile"), "ovcli.profile")?.unwrap_or(false);

    Ok(OvCliConfig {
        url: optional_string(data.get("url"), "ovcli.url")?,
        api_key: optional_string(data.get("api_key"), "ovcli.api_key")?,
        account: optional_string(data.get("account"), "ovcli.account")?,
        user: optional_string(data.get("user"), "ovcli.user")?,
        actor_peer_id,
        agent_id,
        timeout,
        profile,
        extra_headers,
        gateway_token: optional_string(data.get("gateway_token"), "ovcli.gateway_token")?,
        upload_mode,
        output: optional_string(data.get("output"), "ovcli.output")?,
        auth_mode: optional_string(data.get("auth_mode"), "ovcli.auth_mode")?,
        ldap_username: optional_string(data.get("ldap_username"), "ovcli.ldap_username")?,
        ldap_password: optional_string(data.get("ldap_password"), "ovcli.ldap_password")?,
        oidc_token: optional_string(data.get("oidc_token"), "ovcli.oidc_token")?,
    })
}

fn require_mapping<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>> {
    value.as_object().ok_or_else(|| {
        OpenVikingError::config(format!("Invalid value for '{path}': expected object"))
    })
}

fn optional_string(value: Option<&Value>, path: &str) -> Result<Option<String>> {
    let Some(val) = value else {
        return Ok(None);
    };
    if val.is_null() {
        return Ok(None);
    }
    val.as_str().map(|s| Some(s.to_owned())).ok_or_else(|| {
        OpenVikingError::config(format!("Invalid value for '{path}': expected string"))
    })
}

fn optional_bool(value: Option<&Value>, path: &str) -> Result<Option<bool>> {
    let Some(val) = value else {
        return Ok(None);
    };
    if val.is_null() {
        return Ok(None);
    }
    val.as_bool().map(Some).ok_or_else(|| {
        OpenVikingError::config(format!("Invalid value for '{path}': expected boolean"))
    })
}

fn optional_float(value: Option<&Value>, path: &str) -> Result<Option<f64>> {
    let Some(val) = value else {
        return Ok(None);
    };
    if val.is_null() {
        return Ok(None);
    }
    // Note: serde_json::Value::is_boolean is already disjoint from as_f64, so booleans fail cleanly.
    val.as_f64().map(Some).ok_or_else(|| {
        OpenVikingError::config(format!("Invalid value for '{path}': expected number"))
    })
}

fn parse_extra_headers(value: Option<&Value>, path: &str) -> Result<BTreeMap<String, String>> {
    let Some(val) = value else {
        return Ok(BTreeMap::new());
    };
    if val.is_null() {
        return Ok(BTreeMap::new());
    }
    let map = require_mapping(val, path)?;
    let mut parsed = BTreeMap::new();
    for (k, v) in map {
        let Some(header_val) = v.as_str() else {
            return Err(OpenVikingError::config(format!(
                "Invalid value for '{path}.{k}': expected string"
            )));
        };
        parsed.insert(k.clone(), header_val.to_owned());
    }
    Ok(parsed)
}

fn check_unknown_keys(path: &str, data: &Map<String, Value>, allowed: &[&str]) -> Result<()> {
    let allowed_set: BTreeSet<&str> = allowed.iter().copied().collect();
    let mut unknown: Vec<&str> = data
        .keys()
        .map(String::as_str)
        .filter(|key| !allowed_set.contains(key))
        .collect();
    unknown.sort_unstable();
    let Some(first_unknown) = unknown.first().copied() else {
        return Ok(());
    };

    let mut message = format!("Unknown field '{path}.{first_unknown}'");
    if let Some(suggestion) = closest_match(first_unknown, allowed, 0.6) {
        message.push_str(&format!(". Did you mean '{path}.{suggestion}'?"));
    }
    Err(OpenVikingError::config(message))
}

fn closest_match<'a>(needle: &str, candidates: &[&'a str], cutoff: f64) -> Option<&'a str> {
    let mut best: Option<(&'a str, f64)> = None;
    let mut sorted = candidates.to_vec();
    sorted.sort_unstable();
    for candidate in sorted {
        let score = similarity_ratio(needle, candidate);
        if score >= cutoff && best.is_none_or(|(_, prev)| score > prev) {
            best = Some((candidate, score));
        }
    }
    best.map(|(cand, _)| cand)
}

fn similarity_ratio(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let total = a_chars.len() + b_chars.len();
    if total == 0 {
        return 1.0;
    }
    // Longest common subsequence length * 2 / (len(a) + len(b)), matching SequenceMatcher on short keys.
    let mut dp = vec![vec![0usize; b_chars.len() + 1]; a_chars.len() + 1];
    for (i, ca) in a_chars.iter().enumerate() {
        for (j, cb) in b_chars.iter().enumerate() {
            dp[i + 1][j + 1] = if ca == cb {
                dp[i][j] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    (2 * dp[a_chars.len()][b_chars.len()]) as f64 / total as f64
}
