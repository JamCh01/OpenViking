// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Pure-async Rust HTTP SDK for OpenViking, aligned with Python `openviking-sdk`.
//!
//! # Quick Start
//!
//! ```no_run
//! use openviking_sdk::{AsyncHttpClient, FindOptions};
//!
//! # async fn example() -> openviking_sdk::Result<()> {
//! let client = AsyncHttpClient::builder()
//!     .url("http://127.0.0.1:1933")
//!     .api_key("your-user-key")
//!     .build()?;
//!
//! let healthy = client.health().await;
//! println!("healthy: {healthy}");
//!
//! let hits = client.find("deployment preference", FindOptions::default()).await?;
//! println!("hits: {hits:?}");
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented
)]

pub mod actor_peer;
pub mod api;
pub mod arranger;
pub mod client;
pub mod config;
pub mod error;
pub mod message;
pub mod options;
pub mod transport;
pub mod types;
pub mod uploads;
pub mod uri;

pub use crate::actor_peer::{get_actor_peer_id, use_actor_peer};
pub use crate::api::mcps::{McpScopeOptions, MountMcpToolOptions};
pub use crate::api::sessions::{SessionHandle, DEFAULT_SESSION_CONTEXT_TOKEN_BUDGET};
pub use crate::api::snapshot::SnapshotNamespace;
pub use crate::api::system::ObserverNamespace;
pub use crate::arranger::{
    compute_fingerprint, ApplyOptions, AsyncContextArranger, ContextApplyResult, ContextChange,
    ContextChangeAction, ContextPlan, ContextResourceKind, LiveContext, LiveContextOptions,
};
pub use crate::client::{AsyncHttpClient, ClientBuilder, OpenVikingClient};
pub use crate::config::{
    get_basic_auth_header, load_ovcli_config, ClientConfig, ClientConfigBuilder, EnvSource,
    OvCliConfig, SystemEnv, UploadMode, OPENVIKING_CLI_CONFIG_ENV,
};
pub use crate::error::{ApiErrorPayload, ErrorKind, JsonObject, OpenVikingError, Result};
pub use crate::message::{ContextPart, ImagePart, Message, MessagePart, TextPart, ToolPart};
pub use crate::options::{
    AddResourceOptions, AddSkillOptions, AdminListAccountsOptions, AdminListUsersOptions,
    AdminUserInitOptions, AssetGitAuth, BatchAddMessagesOptions, BatchWriteOperation,
    BatchWriteOptions, CommitSessionOptions, CompileOptions, CompileRequest, CreateSessionOptions,
    ExperienceOutcomeOptions, ExperienceTrajectoryOptions, FindOptions, FindSkillsOptions,
    GetSkillOptions, GlobOptions, GrepOptions, ImageInput, ImportPackOptions, ListTasksOptions,
    LsOptions, PreflightAssetOptions, ReindexOptions, RemoveOptions, ResolveAssetsOptions,
    RuleCompileOptions, RuleCompileSource, RuleSearchOptions, SearchContextOptions, SearchOptions,
    SetTagsOptions, SkillSource, SnapshotCommitOptions, SnapshotDiffPathsOptions,
    SnapshotLogOptions, SnapshotRestoreOptions, TreeOptions, UpdateSessionConfigOptions,
    UpdateSkillOptions, UpdateWatchOptions, ValidateSkillOptions, WatchRef, WriteOptions,
};
pub use crate::types::{
    AclEntry, AclSpec, LevelFilter, ListPage, McpToolMount, Nullable, ObserverFormat,
    ProcessingMode, ResponseEnvelope, SearchContextEntry, SearchContextResult, SnapshotBlob,
    SnapshotShowResult, TagMode, TargetUri, TreePage,
};
pub use crate::uploads::zip_directory;
pub use crate::uri::{normalize_uri, VikingUri};
