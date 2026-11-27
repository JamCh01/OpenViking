// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Primary `AsyncHttpClient` (`OpenVikingClient`) definition and builder.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use http::{HeaderMap, Method};
use url::Url;

use crate::api::sessions::SessionHandle;
use crate::api::snapshot::SnapshotNamespace;
use crate::api::system::ObserverNamespace;
use crate::arranger::AsyncContextArranger;
use crate::config::{ClientConfig, ClientConfigBuilder, EnvSource};
use crate::error::Result;
use crate::transport::{RequestHook, Transport};

/// Alias for [`AsyncHttpClient`] matching the naming used across OpenViking SDKs.
pub type OpenVikingClient = AsyncHttpClient;

/// Pure-async HTTP client for an OpenViking server, aligned with Python's `AsyncHTTPClient`.
///
/// Cloning an `AsyncHttpClient` is cheap (`Arc`-backed connection pool and configuration) and safe
/// to share across concurrent Tokio tasks alongside [`crate::use_actor_peer`].
#[derive(Debug, Clone)]
pub struct AsyncHttpClient {
    pub(crate) transport: Transport,
    pub(crate) temp_dir: Option<PathBuf>,
}

impl AsyncHttpClient {
    /// Creates an [`AsyncHttpClient`] from an already-resolved [`ClientConfig`].
    pub fn new(config: ClientConfig) -> Result<Self> {
        Self::with_hooks(config, Vec::new(), None)
    }

    pub(crate) fn with_hooks(
        config: ClientConfig,
        request_hooks: Vec<RequestHook>,
        temp_dir: Option<PathBuf>,
    ) -> Result<Self> {
        let transport = Transport::new(config, request_hooks)?;
        Ok(Self {
            transport,
            temp_dir,
        })
    }

    /// Creates an [`AsyncHttpClient`] resolved purely from `OPENVIKING_*` environment variables
    /// and `~/.openviking/ovcli.conf`.
    pub fn from_env() -> Result<Self> {
        Self::new(ClientConfig::from_env()?)
    }

    /// Creates an [`AsyncHttpClient`] with an explicit server base URL while resolving remaining
    /// settings from environment variables and `~/.openviking/ovcli.conf`.
    pub fn from_url(url: impl Into<String>) -> Result<Self> {
        Self::new(ClientConfig::with_url(url)?)
    }

    /// Starts a fluent [`ClientBuilder`] for configuring and constructing an [`AsyncHttpClient`].
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Returns the normalized base URL of the target OpenViking server.
    pub fn base_url(&self) -> &str {
        self.transport.base_url()
    }

    /// Reports whether request-scoped [`crate::use_actor_peer`] overrides are supported (`true`).
    pub fn supports_request_actor_peer(&self) -> bool {
        true
    }

    /// Async initialization hook kept for parity with Python's `await client.initialize()`.
    ///
    /// In Rust, [`AsyncHttpClient`] initializes its `reqwest` connection pool eagerly during
    /// construction, so calling `initialize` is optional and succeeds immediately.
    pub async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    /// Async teardown hook kept for parity with Python's `await client.close()`.
    pub async fn close(&self) {}

    /// Creates a lightweight [`SessionHandle`] bound to `session_id` without issuing a network call.
    pub fn session(&self, session_id: impl Into<String>) -> SessionHandle {
        SessionHandle::new(self.clone(), session_id.into())
    }

    /// Verifies that `session_id` exists on the server (`GET /api/v1/sessions/{session_id}`) before
    /// returning a [`SessionHandle`], mirroring Python's `client.session(id, must_exist=True)`.
    pub async fn session_checked(&self, session_id: impl Into<String>) -> Result<SessionHandle> {
        let id = session_id.into();
        if !id.is_empty() {
            let _ = self.get_session(&id, false).await?;
        }
        Ok(SessionHandle::new(self.clone(), id))
    }

    /// Returns the async observer sub-namespace (`queue`, `vikingdb`, `models`, `system`, `is_healthy`).
    pub fn observer(&self) -> ObserverNamespace {
        ObserverNamespace::new(self.clone())
    }

    /// Returns the async snapshot version-control sub-namespace (`commit`, `restore`, `show`, `log`,
    /// `diff`, `diff_paths`, `get_gitignore`, `set_gitignore`, `delete_gitignore`).
    pub fn snapshot(&self) -> SnapshotNamespace {
        SnapshotNamespace::new(self.clone())
    }

    /// Creates a declarative [`AsyncContextArranger`] bound to this client.
    pub fn arranger(&self, name: &str, root_uri: Option<&str>) -> Result<AsyncContextArranger> {
        AsyncContextArranger::new(self.clone(), name, root_uri.unwrap_or("viking://resources"))
    }

    pub(crate) fn temp_dir_path(&self) -> Option<&Path> {
        self.temp_dir.as_deref()
    }
}

/// Fluent builder constructing an [`AsyncHttpClient`].
#[derive(Default)]
pub struct ClientBuilder {
    inner: ClientConfigBuilder,
    request_hooks: Vec<RequestHook>,
    temp_dir: Option<PathBuf>,
}

impl std::fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("inner", &self.inner)
            .field("request_hooks_len", &self.request_hooks.len())
            .field("temp_dir", &self.temp_dir)
            .finish()
    }
}

impl ClientBuilder {
    /// Sets the OpenViking server base URL.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.inner = self.inner.url(url);
        self
    }

    /// Sets the API key (`X-API-Key`).
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.inner = self.inner.api_key(api_key);
        self
    }

    /// Sets the tenant account header (`X-OpenViking-Account`).
    pub fn account(mut self, account: impl Into<String>) -> Self {
        self.inner = self.inner.account(account);
        self
    }

    /// Sets the tenant user header (`X-OpenViking-User`).
    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.inner = self.inner.user(user);
        self
    }

    /// Sets the legacy `user_id` alias.
    pub fn user_id(mut self, user_id: impl Into<String>) -> Self {
        self.inner = self.inner.user_id(user_id);
        self
    }

    /// Sets the default actor peer header (`X-OpenViking-Actor-Peer`).
    pub fn actor_peer_id(mut self, actor_peer_id: impl Into<String>) -> Self {
        self.inner = self.inner.actor_peer_id(actor_peer_id);
        self
    }

    /// Sets the legacy `agent_id` alias (mutually exclusive with `actor_peer_id`).
    pub fn agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.inner = self.inner.agent_id(agent_id);
        self
    }

    /// Sets the request timeout duration.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.inner = self.inner.timeout(timeout);
        self
    }

    /// Sets the request timeout in seconds.
    pub fn timeout_secs(mut self, seconds: f64) -> Self {
        self.inner = self.inner.timeout_secs(seconds);
        self
    }

    /// Sets static extra HTTP headers.
    pub fn extra_headers<I, K, V>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.inner = self.inner.extra_headers(headers);
        self
    }

    /// Enables or disables `?profile=1` query attachment.
    pub fn profile_enabled(mut self, enabled: bool) -> Self {
        self.inner = self.inner.profile_enabled(enabled);
        self
    }

    /// Sets the temporary upload mode (`"local"` or `"shared"`).
    pub fn upload_mode(mut self, mode: impl Into<String>) -> Self {
        self.inner = self.inner.upload_mode(mode);
        self
    }

    /// Sets an explicit gateway token.
    pub fn gateway_token(mut self, token: impl Into<String>) -> Self {
        self.inner = self.inner.gateway_token(token);
        self
    }

    /// Sets the authentication mode (`"ldap"`, `"oidc"`, or `"api_key"`).
    pub fn auth_mode(mut self, mode: impl Into<String>) -> Self {
        self.inner = self.inner.auth_mode(mode);
        self
    }

    /// Sets the LDAP username.
    pub fn ldap_username(mut self, username: impl Into<String>) -> Self {
        self.inner = self.inner.ldap_username(username);
        self
    }

    /// Sets the LDAP password.
    pub fn ldap_password(mut self, password: impl Into<String>) -> Self {
        self.inner = self.inner.ldap_password(password);
        self
    }

    /// Sets the OIDC bearer token.
    pub fn oidc_token(mut self, token: impl Into<String>) -> Self {
        self.inner = self.inner.oidc_token(token);
        self
    }

    /// Overrides the `ovcli.conf` file path.
    pub fn config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.inner = self.inner.config_path(path);
        self
    }

    /// Disables reading any `ovcli.conf` file from disk.
    pub fn without_config_file(mut self) -> Self {
        self.inner = self.inner.without_config_file();
        self
    }

    /// Supplies a custom `reqwest::Client`.
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.inner = self.inner.http_client(client);
        self
    }

    /// Registers a request hook invoked before each outgoing HTTP request (analogous to Python `event_hooks`).
    pub fn request_hook<F>(mut self, hook: F) -> Self
    where
        F: Fn(&Method, &Url, &mut HeaderMap) + Send + Sync + 'static,
    {
        self.request_hooks.push(Arc::new(hook));
        self
    }

    /// Overrides the scratch directory used when creating temporary `.zip` archives for directory uploads.
    pub fn temp_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.temp_dir = Some(dir.into());
        self
    }

    /// Resolves configuration from the system environment and builds the [`AsyncHttpClient`].
    pub fn build(self) -> Result<AsyncHttpClient> {
        let config = self.inner.build()?;
        AsyncHttpClient::with_hooks(config, self.request_hooks, self.temp_dir)
    }

    /// Resolves configuration using a custom [`EnvSource`] and builds the [`AsyncHttpClient`].
    pub fn build_with_env(self, env: &dyn EnvSource) -> Result<AsyncHttpClient> {
        let config = self.inner.build_with_env(env)?;
        AsyncHttpClient::with_hooks(config, self.request_hooks, self.temp_dir)
    }
}
