// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Declarative Context-as-Code orchestration (`AsyncContextArranger`), aligned with Python `arranger.py`.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::client::AsyncHttpClient;
use crate::error::{JsonObject, OpenVikingError, Result};
use crate::options::{LsOptions, RemoveOptions};
use crate::types::McpToolMount;
use crate::uri::VikingUri;

const OWNER_TAG_KEY: &str = "ov-cac-owner";
const KIND_TAG_KEY: &str = "ov-cac-kind";
const SPEC_TAG_KEY: &str = "ov-cac-spec";
const RESERVED_TAG_KEYS: &[&str] = &[OWNER_TAG_KEY, KIND_TAG_KEY, SPEC_TAG_KEY];
const DEFAULT_MAX_MANAGED_RESOURCES: usize = 10_000;

/// Kind of declarative context resource managed by [`AsyncContextArranger`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextResourceKind {
    /// Live context node backed by an MCP tool (`"live_context"`).
    #[serde(rename = "live_context")]
    LiveContext,
}

impl ContextResourceKind {
    /// Returns the wire string for this resource kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LiveContext => "live_context",
        }
    }
}

/// Planned reconciliation action for a managed context node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContextChangeAction {
    /// Target does not exist and will be created.
    #[serde(rename = "create")]
    Create,
    /// Managed target differs from declaration and will be updated.
    #[serde(rename = "update")]
    Update,
    /// Managed target is absent from declaration and eligible for pruning.
    #[serde(rename = "delete")]
    Delete,
    /// Target matches the declared specification.
    #[serde(rename = "unchanged")]
    Unchanged,
    /// Target exists unmanaged or has an incompatible kind.
    #[serde(rename = "conflict")]
    Conflict,
}

impl ContextChangeAction {
    /// Returns the wire string for this action.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Unchanged => "unchanged",
            Self::Conflict => "conflict",
        }
    }

    const ALL: [Self; 5] = [
        Self::Create,
        Self::Update,
        Self::Delete,
        Self::Unchanged,
        Self::Conflict,
    ];
}

/// Optional fields when declaring a [`LiveContext`] resource on an [`AsyncContextArranger`].
#[derive(Debug, Clone)]
pub struct LiveContextOptions {
    /// Optional human-readable description.
    pub description: Option<String>,
    /// Evaluation timeout in seconds (defaults to 60; must be positive).
    pub timeout_seconds: i64,
    /// Tool arguments passed at evaluation time.
    pub arguments: JsonObject,
    /// User-defined `k=v` tags (normalized and sorted).
    pub tags: Vec<String>,
}

impl Default for LiveContextOptions {
    fn default() -> Self {
        Self {
            description: None,
            timeout_seconds: 60,
            arguments: JsonObject::new(),
            tags: Vec::new(),
        }
    }
}

/// Immutable declaration of an MCP-backed Live Context resource.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveContext {
    /// Canonical destination `viking://.../name.md` URI.
    pub uri: String,
    /// Canonical MCP tool URI `viking://agent/tools/<server>/<tool>.json`.
    pub tool_uri: String,
    /// Node title.
    pub title: String,
    /// Optional description.
    pub description: Option<String>,
    /// Evaluation timeout in seconds.
    pub timeout_seconds: i64,
    /// Tool invocation arguments.
    pub arguments: JsonObject,
    /// Normalized user tags (`k=v`).
    pub tags: Vec<String>,
}

impl LiveContext {
    /// Returns [`ContextResourceKind::LiveContext`].
    pub fn kind(&self) -> ContextResourceKind {
        ContextResourceKind::LiveContext
    }
}

/// One planned or applied change in a [`ContextPlan`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextChange {
    /// Planned action.
    pub action: ContextChangeAction,
    /// Resource kind.
    pub kind: ContextResourceKind,
    /// Target `viking://` URI.
    pub uri: String,
    /// Human-readable reason.
    pub reason: String,
}

impl ContextChange {
    /// Serializes this change into a JSON dictionary matching Python `ContextChange.to_dict()`.
    pub fn to_dict(&self) -> JsonObject {
        let mut map = Map::new();
        map.insert(
            "action".to_owned(),
            Value::String(self.action.as_str().to_owned()),
        );
        map.insert(
            "kind".to_owned(),
            Value::String(self.kind.as_str().to_owned()),
        );
        map.insert("uri".to_owned(), Value::String(self.uri.clone()));
        map.insert("reason".to_owned(), Value::String(self.reason.clone()));
        map
    }
}

/// Computed reconciliation plan produced by [`AsyncContextArranger::plan`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextPlan {
    /// Normalized project owner name.
    pub project: String,
    /// Root `viking://` URI managed by this arranger.
    pub root_uri: String,
    /// Ordered list of changes.
    pub changes: Vec<ContextChange>,
}

impl ContextPlan {
    /// Reports whether any `create`, `update`, or `delete` action is present.
    pub fn has_changes(&self) -> bool {
        self.changes.iter().any(|c| {
            matches!(
                c.action,
                ContextChangeAction::Create
                    | ContextChangeAction::Update
                    | ContextChangeAction::Delete
            )
        })
    }

    /// Reports whether any `conflict` entry prevents applying this plan.
    pub fn has_conflicts(&self) -> bool {
        self.changes
            .iter()
            .any(|c| c.action == ContextChangeAction::Conflict)
    }

    /// Converts this plan into the structured JSON object returned by Python `ContextPlan.to_dict()`.
    pub fn to_dict(&self) -> JsonObject {
        let mut counts = Map::new();
        for action in ContextChangeAction::ALL {
            counts.insert(action.as_str().to_owned(), Value::Number(0.into()));
        }
        for change in &self.changes {
            let key = change.action.as_str();
            let next = counts.get(key).and_then(Value::as_i64).unwrap_or(0) + 1;
            counts.insert(key.to_owned(), Value::Number(next.into()));
        }

        let mut map = Map::new();
        map.insert("project".to_owned(), Value::String(self.project.clone()));
        map.insert("root_uri".to_owned(), Value::String(self.root_uri.clone()));
        map.insert("has_changes".to_owned(), Value::Bool(self.has_changes()));
        map.insert(
            "has_conflicts".to_owned(),
            Value::Bool(self.has_conflicts()),
        );
        map.insert("counts".to_owned(), Value::Object(counts));
        map.insert(
            "changes".to_owned(),
            Value::Array(
                self.changes
                    .iter()
                    .map(|c| Value::Object(c.to_dict()))
                    .collect(),
            ),
        );
        map
    }

    /// Formats this plan as a human-readable ASCII summary table.
    pub fn format_table(&self) -> String {
        format_summary_table(self, None)
    }
}

/// Options controlling [`AsyncContextArranger::apply`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ApplyOptions {
    /// Delete owned resources absent from the current declaration.
    pub prune: bool,
    /// Compute and validate the plan without mutating OpenViking state.
    pub dry_run: bool,
    /// Wait for mount/unmount background indexing to finish.
    pub wait: bool,
}

/// Reconciliation outcome returned by [`AsyncContextArranger::apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextApplyResult {
    /// Computed plan.
    pub plan: ContextPlan,
    /// Whether pruning was enabled.
    pub prune: bool,
    /// Whether `dry_run` was enabled.
    pub dry_run: bool,
    /// Changes actually applied to the server.
    pub applied: Vec<ContextChange>,
}

impl ContextApplyResult {
    /// Converts this result into the structured JSON object matching Python `ContextApplyResult.to_dict()`.
    pub fn to_dict(&self) -> JsonObject {
        let mut map = Map::new();
        map.insert("prune".to_owned(), Value::Bool(self.prune));
        map.insert("dry_run".to_owned(), Value::Bool(self.dry_run));
        map.insert(
            "applied".to_owned(),
            Value::Array(
                self.applied
                    .iter()
                    .map(|c| Value::Object(c.to_dict()))
                    .collect(),
            ),
        );
        map.insert("plan".to_owned(), Value::Object(self.plan.to_dict()));
        map
    }

    /// Formats this apply result as a human-readable ASCII summary table.
    pub fn format_table(&self) -> String {
        format_summary_table(&self.plan, Some(self))
    }
}

/// Pure-async declarative Context-as-Code orchestrator aligned with Python's `AsyncContextArranger`.
#[derive(Debug, Clone)]
pub struct AsyncContextArranger {
    client: AsyncHttpClient,
    name: String,
    root_uri: String,
    max_managed_resources: usize,
    resources: BTreeMap<String, LiveContext>,
}

impl AsyncContextArranger {
    /// Creates a new [`AsyncContextArranger`] managing `root_uri` under project owner `name`.
    pub fn new(client: AsyncHttpClient, name: &str, root_uri: &str) -> Result<Self> {
        Self::with_max_managed_resources(client, name, root_uri, DEFAULT_MAX_MANAGED_RESOURCES)
    }

    /// Creates a new [`AsyncContextArranger`] with a custom `max_managed_resources` safety cap.
    pub fn with_max_managed_resources(
        client: AsyncHttpClient,
        name: &str,
        root_uri: &str,
        max_managed_resources: usize,
    ) -> Result<Self> {
        let normalized_name = name.trim().to_ascii_lowercase();
        if !is_valid_owner_name(&normalized_name) {
            return Err(OpenVikingError::invalid_argument(
                "Context project name must contain only lowercase letters, digits, '.', '_' or '-'",
            ));
        }
        if max_managed_resources == 0 {
            return Err(OpenVikingError::invalid_argument(
                "max_managed_resources must be positive",
            ));
        }

        let normalized_root = VikingUri::normalize(root_uri)
            .trim_end_matches('/')
            .to_owned();
        if !is_context_resource_root(&normalized_root) {
            return Err(OpenVikingError::invalid_argument(
                "Context project root_uri must be within viking://resources or viking://user/<user_id>/resources",
            ));
        }

        Ok(Self {
            client,
            name: normalized_name,
            root_uri: normalized_root,
            max_managed_resources,
            resources: BTreeMap::new(),
        })
    }

    /// Returns the project owner name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the managed root URI.
    pub fn root_uri(&self) -> &str {
        &self.root_uri
    }

    /// Returns the `ov-cac-owner=<name>` tag used to identify managed nodes.
    pub fn owner_tag(&self) -> String {
        format!("{OWNER_TAG_KEY}={}", self.name)
    }

    /// Returns the declared [`LiveContext`] resources sorted by URI.
    pub fn resources(&self) -> Vec<&LiveContext> {
        self.resources.values().collect()
    }

    /// Declares an MCP-backed [`LiveContext`] node under `self.root_uri()`.
    pub fn live_context(
        &mut self,
        path: &str,
        tool_uri: &str,
        title: &str,
        options: LiveContextOptions,
    ) -> Result<&mut Self> {
        let uri = self.resolve_uri(path)?;
        let normalized_tool_uri = VikingUri::normalize(tool_uri);
        if !uri.ends_with(".md") {
            return Err(OpenVikingError::invalid_argument(
                "Live context path must end with .md",
            ));
        }
        if parse_agent_tool_uri(&normalized_tool_uri).is_none() {
            return Err(OpenVikingError::invalid_argument(
                "tool_uri must match viking://agent/tools/<server>/<tool>.json",
            ));
        }
        if title.trim().is_empty() {
            return Err(OpenVikingError::invalid_argument(
                "Live context title must not be empty",
            ));
        }
        if options.timeout_seconds <= 0 {
            return Err(OpenVikingError::invalid_argument(
                "timeout_seconds must be positive",
            ));
        }
        if self.resources.contains_key(&uri) {
            return Err(OpenVikingError::invalid_argument(format!(
                "Context resource declared more than once: {uri}"
            )));
        }

        let normalized_tags = normalize_user_tags(&options.tags)?;
        let resource = LiveContext {
            uri: uri.clone(),
            tool_uri: normalized_tool_uri,
            title: title.to_owned(),
            description: options.description,
            timeout_seconds: options.timeout_seconds,
            arguments: options.arguments,
            tags: normalized_tags,
        };
        self.resources.insert(uri, resource);
        Ok(self)
    }

    /// Computes the reconciliation [`ContextPlan`] without modifying OpenViking state.
    pub async fn plan(&self) -> Result<ContextPlan> {
        let managed = self.managed_entries().await?;
        let mut changes = Vec::new();

        for (uri, resource) in &self.resources {
            let fingerprint = compute_fingerprint(resource)?;
            let Some(entry) = managed.get(uri) else {
                if self.uri_exists(uri).await? {
                    changes.push(ContextChange {
                        action: ContextChangeAction::Conflict,
                        kind: resource.kind(),
                        uri: uri.clone(),
                        reason: "target exists but is not managed by this project".to_owned(),
                    });
                } else {
                    changes.push(ContextChange {
                        action: ContextChangeAction::Create,
                        kind: resource.kind(),
                        uri: uri.clone(),
                        reason: "target does not exist".to_owned(),
                    });
                }
                continue;
            };

            let tags = entry_tags_by_key(entry);
            if let Some(current_kind) = tags.get(KIND_TAG_KEY) {
                if current_kind != resource.kind().as_str() {
                    changes.push(ContextChange {
                        action: ContextChangeAction::Conflict,
                        kind: resource.kind(),
                        uri: uri.clone(),
                        reason: format!("managed target kind is {current_kind}"),
                    });
                    continue;
                }
            }

            let spec_matches = tags
                .get(SPEC_TAG_KEY)
                .is_some_and(|spec| spec == &fingerprint);
            if spec_matches && self.matches_applied(resource, entry, &fingerprint).await? {
                changes.push(ContextChange {
                    action: ContextChangeAction::Unchanged,
                    kind: resource.kind(),
                    uri: uri.clone(),
                    reason: "declaration matches the applied specification".to_owned(),
                });
            } else {
                changes.push(ContextChange {
                    action: ContextChangeAction::Update,
                    kind: resource.kind(),
                    uri: uri.clone(),
                    reason: "actual state differs from the declaration".to_owned(),
                });
            }
        }

        for (uri, entry) in &managed {
            if self.resources.contains_key(uri) {
                continue;
            }
            let tags = entry_tags_by_key(entry);
            let raw_kind = tags.get(KIND_TAG_KEY).map(String::as_str);
            if raw_kind != Some(ContextResourceKind::LiveContext.as_str()) {
                changes.push(ContextChange {
                    action: ContextChangeAction::Conflict,
                    kind: ContextResourceKind::LiveContext,
                    uri: uri.clone(),
                    reason: format!("managed target has unsupported kind {raw_kind:?}"),
                });
                continue;
            }
            changes.push(ContextChange {
                action: ContextChangeAction::Delete,
                kind: ContextResourceKind::LiveContext,
                uri: uri.clone(),
                reason: "managed target is absent from the declaration".to_owned(),
            });
        }

        Ok(ContextPlan {
            project: self.name.clone(),
            root_uri: self.root_uri.clone(),
            changes,
        })
    }

    /// Converges OpenViking state toward the declared specification.
    pub async fn apply(&self, options: ApplyOptions) -> Result<ContextApplyResult> {
        let plan = self.plan().await?;
        if plan.has_conflicts() {
            let conflicts: Vec<Value> = plan
                .changes
                .iter()
                .filter(|c| c.action == ContextChangeAction::Conflict)
                .map(|c| Value::Object(c.to_dict()))
                .collect();
            let mut details = Map::new();
            details.insert("conflicts".to_owned(), Value::Array(conflicts));
            return Err(OpenVikingError::failed_precondition(
                "Context plan contains conflicts",
                details,
            ));
        }

        if options.dry_run {
            return Ok(ContextApplyResult {
                plan,
                prune: options.prune,
                dry_run: true,
                applied: Vec::new(),
            });
        }

        let mut applied = Vec::new();
        let mut mount_changes = Vec::new();
        let mut mounts = Vec::new();
        let owner_tag = self.owner_tag();

        for change in &plan.changes {
            if !matches!(
                change.action,
                ContextChangeAction::Create | ContextChangeAction::Update
            ) {
                continue;
            }
            let Some(resource) = self.resources.get(&change.uri) else {
                continue;
            };
            let tags = build_managed_tags(&owner_tag, resource)?;
            mounts.push(McpToolMount {
                tool_uri: resource.tool_uri.clone(),
                node_uri: resource.uri.clone(),
                title: resource.title.clone(),
                description: resource.description.clone(),
                timeout_seconds: Some(resource.timeout_seconds),
                arguments: resource.arguments.clone(),
                tags: Some(tags),
            });
            mount_changes.push(change.clone());
        }

        if !mounts.is_empty() {
            let batch_res = self
                .client
                .batch_mount_mcp_tools(&mounts, options.wait)
                .await?;
            check_batch_result(&batch_res, &mount_changes, &applied)?;
            applied.extend(mount_changes);
        }

        if options.prune {
            let delete_changes: Vec<ContextChange> = plan
                .changes
                .iter()
                .filter(|c| c.action == ContextChangeAction::Delete)
                .cloned()
                .collect();

            if !delete_changes.is_empty() {
                let delete_uris: Vec<String> =
                    delete_changes.iter().map(|c| c.uri.clone()).collect();
                match self
                    .client
                    .batch_unmount_mcp_tools(&delete_uris, options.wait)
                    .await
                {
                    Ok(batch_res) => {
                        check_batch_result(&batch_res, &delete_changes, &applied)?;
                    }
                    Err(err) if err.is_not_found() => {
                        for change in &delete_changes {
                            match self
                                .client
                                .unmount_mcp_tool(&change.uri, options.wait)
                                .await
                            {
                                Ok(_) => {}
                                Err(unmount_err) if unmount_err.is_not_found() => {
                                    self.client
                                        .rm(
                                            &change.uri,
                                            RemoveOptions {
                                                recursive: false,
                                                wait: options.wait,
                                            },
                                        )
                                        .await?;
                                }
                                Err(unmount_err) => return Err(unmount_err),
                            }
                        }
                    }
                    Err(err) => return Err(err),
                }
                applied.extend(delete_changes);
            }
        }

        Ok(ContextApplyResult {
            plan,
            prune: options.prune,
            dry_run: false,
            applied,
        })
    }

    fn resolve_uri(&self, path: &str) -> Result<String> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err(OpenVikingError::invalid_argument(
                "Context resource path must not be empty",
            ));
        }
        let uri = if trimmed.starts_with("viking://") {
            VikingUri::normalize(trimmed)
                .trim_end_matches('/')
                .to_owned()
        } else {
            let stripped = trimmed.trim_matches('/');
            let parts: Vec<&str> = stripped.split('/').collect();
            if parts
                .iter()
                .any(|p| p.is_empty() || *p == "." || *p == "..")
            {
                return Err(OpenVikingError::invalid_argument(format!(
                    "Invalid context resource path: {path}"
                )));
            }
            format!("{}/{}", self.root_uri, parts.join("/"))
        };

        let required_prefix = format!("{}/", self.root_uri);
        if uri == self.root_uri || !uri.starts_with(&required_prefix) {
            return Err(OpenVikingError::invalid_argument(format!(
                "Context resource must be below {}: {uri}",
                self.root_uri
            )));
        }
        Ok(uri)
    }

    async fn managed_entries(&self) -> Result<BTreeMap<String, JsonObject>> {
        let ls_opts = LsOptions {
            recursive: true,
            include_tags: true,
            tags: Some(vec![self.owner_tag()]),
            node_limit: self.max_managed_resources,
            ..LsOptions::default()
        };
        let entries = match self.client.ls(&self.root_uri, ls_opts).await {
            Ok(items) => items,
            Err(err) if err.is_not_found() => return Ok(BTreeMap::new()),
            Err(err) => return Err(err),
        };

        if entries.len() >= self.max_managed_resources {
            return Err(OpenVikingError::failed_precondition(
                "Managed resource scan reached max_managed_resources; increase the limit before planning or pruning",
                Map::new(),
            ));
        }

        let mut out = BTreeMap::new();
        for item in entries {
            let Some(obj) = item.as_object() else {
                continue;
            };
            if obj.get("isDir").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            if let Some(uri) = obj.get("uri").and_then(Value::as_str) {
                out.insert(VikingUri::normalize(uri), obj.clone());
            }
        }
        Ok(out)
    }

    async fn uri_exists(&self, uri: &str) -> Result<bool> {
        match self.client.stat(uri).await {
            Ok(_) => Ok(true),
            Err(err) if err.is_not_found() => Ok(false),
            Err(err) => Err(err),
        }
    }

    async fn matches_applied(
        &self,
        resource: &LiveContext,
        entry: &JsonObject,
        fingerprint: &str,
    ) -> Result<bool> {
        let expected_tags = tags_slice_by_key(&build_managed_tags(&self.owner_tag(), resource)?);
        if entry_tags_by_key(entry) != expected_tags {
            return Ok(false);
        }
        if expected_tags.get(SPEC_TAG_KEY).map(String::as_str) != Some(fingerprint) {
            return Ok(false);
        }

        let Some((server, tool)) = parse_agent_tool_uri(&resource.tool_uri) else {
            return Ok(false);
        };
        let mounts_res = match self.client.list_mcp_mounts(server, Some(tool), None).await {
            Ok(res) => res,
            Err(err) if err.is_not_found() => return Ok(false),
            Err(err) => return Err(err),
        };

        let Some(mounts) = mounts_res.get("mounts").and_then(Value::as_array) else {
            return Ok(false);
        };
        for mount in mounts {
            let Some(obj) = mount.as_object() else {
                continue;
            };
            let node_uri = obj
                .get("node_uri")
                .and_then(Value::as_str)
                .map(VikingUri::normalize)
                .unwrap_or_default();
            if node_uri != resource.uri {
                continue;
            }
            let tool_uri = obj
                .get("tool_uri")
                .and_then(Value::as_str)
                .map(VikingUri::normalize)
                .unwrap_or_default();
            let empty_args = Map::new();
            let actual_args = obj
                .get("arguments")
                .and_then(Value::as_object)
                .unwrap_or(&empty_args);
            return Ok(tool_uri == resource.tool_uri && actual_args == &resource.arguments);
        }
        Ok(false)
    }
}

fn is_valid_owner_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

fn is_valid_user_id(user_id: &str) -> bool {
    !user_id.is_empty()
        && user_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '@' | '-'))
}

fn is_context_resource_root(uri: &str) -> bool {
    let Some(rest) = uri.strip_prefix("viking://") else {
        return false;
    };
    let parts: Vec<&str> = rest.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == "..")
    {
        return false;
    }
    if parts.first() == Some(&"resources") {
        return true;
    }
    parts.len() >= 3 && parts[0] == "user" && is_valid_user_id(parts[1]) && parts[2] == "resources"
}

fn parse_agent_tool_uri(uri: &str) -> Option<(&str, &str)> {
    let rest = uri.strip_prefix("viking://agent/tools/")?;
    let (server, tool_file) = rest.split_once('/')?;
    if server.is_empty() || tool_file.contains('/') {
        return None;
    }
    let tool = tool_file.strip_suffix(".json")?;
    if tool.is_empty() {
        return None;
    }
    Some((server, tool))
}

fn normalize_user_tags(tags: &[String]) -> Result<Vec<String>> {
    let mut map = BTreeMap::new();
    for raw in tags {
        let normalized = raw.trim().to_ascii_lowercase();
        let mut parts = normalized.split('=');
        let Some(key) = parts.next() else {
            return Err(OpenVikingError::invalid_argument(format!(
                "Context tag must use strict k=v format: {raw}"
            )));
        };
        let Some(val) = parts.next() else {
            return Err(OpenVikingError::invalid_argument(format!(
                "Context tag must use strict k=v format: {raw}"
            )));
        };
        if parts.next().is_some() {
            return Err(OpenVikingError::invalid_argument(format!(
                "Context tag must use strict k=v format: {raw}"
            )));
        }
        if key.is_empty() || val.is_empty() {
            return Err(OpenVikingError::invalid_argument(format!(
                "Context tag key and value must be non-empty: {raw}"
            )));
        }
        if RESERVED_TAG_KEYS.contains(&key) {
            return Err(OpenVikingError::invalid_argument(format!(
                "Context tag key is reserved: {key}"
            )));
        }
        map.insert(key.to_owned(), val.to_owned());
    }
    Ok(map.into_iter().map(|(k, v)| format!("{k}={v}")).collect())
}

fn entry_tags_by_key(entry: &JsonObject) -> BTreeMap<String, String> {
    let Some(arr) = entry.get("tags").and_then(Value::as_array) else {
        return BTreeMap::new();
    };
    let strings: Vec<String> = arr
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    tags_slice_by_key(&strings)
}

fn tags_slice_by_key(tags: &[String]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for raw in tags {
        let normalized = raw.trim().to_ascii_lowercase();
        let mut iter = normalized.split('=');
        if let (Some(k), Some(v), None) = (iter.next(), iter.next(), iter.next()) {
            out.insert(k.to_owned(), v.to_owned());
        }
    }
    out
}

/// Computes the exact SHA-256 hex digest produced by Python's `_fingerprint(resource)`.
pub fn compute_fingerprint(resource: &LiveContext) -> Result<String> {
    // serde_json::Map without `preserve_order` is backed by BTreeMap, serializing object keys
    // sorted lexicographically with compact `,` and `:` separators and unescaped UTF-8 — identical
    // to Python `json.dumps(..., ensure_ascii=False, sort_keys=True, separators=(",", ":"))`.
    let mut spec = Map::new();
    spec.insert(
        "arguments".to_owned(),
        canonicalize_json(&Value::Object(resource.arguments.clone())),
    );
    spec.insert(
        "description".to_owned(),
        match &resource.description {
            Some(desc) => Value::String(desc.clone()),
            None => Value::Null,
        },
    );
    spec.insert(
        "kind".to_owned(),
        Value::String(resource.kind().as_str().to_owned()),
    );
    spec.insert(
        "tags".to_owned(),
        Value::Array(resource.tags.iter().cloned().map(Value::String).collect()),
    );
    spec.insert(
        "timeout_seconds".to_owned(),
        Value::Number(resource.timeout_seconds.into()),
    );
    spec.insert("title".to_owned(), Value::String(resource.title.clone()));
    spec.insert(
        "tool_uri".to_owned(),
        Value::String(resource.tool_uri.clone()),
    );
    spec.insert("uri".to_owned(), Value::String(resource.uri.clone()));

    let encoded = serde_json::to_vec(&Value::Object(spec))?;
    let mut hasher = Sha256::new();
    hasher.update(&encoded);
    Ok(hex::encode(hasher.finalize()))
}

fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted: BTreeMap<String, Value> = BTreeMap::new();
            for (k, v) in map {
                sorted.insert(k.clone(), canonicalize_json(v));
            }
            let mut out = Map::new();
            for (k, v) in sorted {
                out.insert(k, v);
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_json).collect()),
        other => other.clone(),
    }
}

fn build_managed_tags(owner_tag: &str, resource: &LiveContext) -> Result<Vec<String>> {
    let fingerprint = compute_fingerprint(resource)?;
    let mut tags = Vec::with_capacity(resource.tags.len() + 3);
    tags.extend(resource.tags.iter().cloned());
    tags.push(owner_tag.to_owned());
    tags.push(format!("{KIND_TAG_KEY}={}", resource.kind().as_str()));
    tags.push(format!("{SPEC_TAG_KEY}={fingerprint}"));
    Ok(tags)
}

fn check_batch_result(
    result: &JsonObject,
    changes: &[ContextChange],
    previously_applied: &[ContextChange],
) -> Result<()> {
    let has_failed = result
        .get("failed")
        .and_then(Value::as_array)
        .is_some_and(|arr| !arr.is_empty());
    let has_not_attempted = result
        .get("not_attempted")
        .and_then(Value::as_array)
        .is_some_and(|arr| !arr.is_empty());
    if !has_failed && !has_not_attempted {
        return Ok(());
    }

    let mut completed: BTreeSet<String> = result
        .get("succeeded")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    if let Some(failed_arr) = result.get("failed").and_then(Value::as_array) {
        for item in failed_arr {
            if let Some(obj) = item.as_object() {
                if obj.get("applied").and_then(Value::as_bool) == Some(true) {
                    if let Some(uri) = obj.get("node_uri").and_then(Value::as_str) {
                        completed.insert(uri.to_owned());
                    }
                }
            }
        }
    }

    let mut applied_list: Vec<Value> = previously_applied
        .iter()
        .map(|c| Value::Object(c.to_dict()))
        .collect();
    for change in changes {
        if completed.contains(&change.uri) {
            applied_list.push(Value::Object(change.to_dict()));
        }
    }

    let mut details = Map::new();
    details.insert("batch_result".to_owned(), Value::Object(result.clone()));
    details.insert("applied".to_owned(), Value::Array(applied_list));
    Err(OpenVikingError::failed_precondition(
        "Context batch partially applied",
        details,
    ))
}

fn format_summary_table(plan: &ContextPlan, apply: Option<&ContextApplyResult>) -> String {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for action in ContextChangeAction::ALL {
        counts.insert(action.as_str(), 0);
    }
    for change in &plan.changes {
        if let Some(slot) = counts.get_mut(change.action.as_str()) {
            *slot += 1;
        }
    }

    let title = match apply {
        Some(res) if res.dry_run => "OpenViking Context Apply (dry run)",
        Some(_) => "OpenViking Context Apply Result",
        None => "OpenViking Context Plan",
    };

    let mut lines = vec![
        title.to_owned(),
        format!("Arrangement: {}", plan.project),
        format!("Root:        {}", plan.root_uri),
    ];

    if let Some(res) = apply {
        lines.push(format!(
            "Prune:       {}",
            if res.prune { "yes" } else { "no" }
        ));
        lines.push(format!(
            "Dry run:     {}",
            if res.dry_run { "yes" } else { "no" }
        ));
        lines.push(format!("Applied:     {}", res.applied.len()));
    }

    let create_n = counts.get("create").copied().unwrap_or(0);
    let update_n = counts.get("update").copied().unwrap_or(0);
    let delete_n = counts.get("delete").copied().unwrap_or(0);
    let unchanged_n = counts.get("unchanged").copied().unwrap_or(0);
    let conflict_n = counts.get("conflict").copied().unwrap_or(0);

    if let Some(res) = apply.filter(|r| !r.dry_run) {
        let delete_summary = if res.prune {
            format!("{delete_n} deleted")
        } else {
            format!("{delete_n} pending deletion")
        };
        lines.push(format!(
            "Summary:     {create_n} created, {update_n} updated, {delete_summary}, {unchanged_n} unchanged"
        ));
    } else {
        lines.push(format!(
            "Summary:     {create_n} create, {update_n} update, {delete_n} delete, {unchanged_n} unchanged, {conflict_n} conflict"
        ));
    }
    lines.push(String::new());

    if plan.changes.is_empty() {
        lines.push("No resources declared.".to_owned());
    } else {
        let is_applied = apply.is_some_and(|r| !r.dry_run);
        let prune = apply.is_some_and(|r| r.prune);
        let header_first = if is_applied { "RESULT" } else { "ACTION" };
        lines.push(format!(
            "{header_first:<14}  {: <12}  {: <40}  DETAIL",
            "TYPE", "URI"
        ));
        lines.push(format!(
            "{}  {}  {}  {}",
            "-".repeat(14),
            "-".repeat(12),
            "-".repeat(40),
            "-".repeat(32)
        ));
        for change in &plan.changes {
            let (label, detail) = if is_applied {
                match change.action {
                    ContextChangeAction::Create => ("CREATED", "created successfully"),
                    ContextChangeAction::Update => ("UPDATED", "updated successfully"),
                    ContextChangeAction::Delete if prune => ("DELETED", "deleted successfully"),
                    ContextChangeAction::Delete => {
                        ("PENDING DELETE", "run apply --prune to delete")
                    }
                    ContextChangeAction::Unchanged => ("UNCHANGED", "already matches declaration"),
                    ContextChangeAction::Conflict => ("CONFLICT", "not applied"),
                }
            } else {
                (
                    match change.action {
                        ContextChangeAction::Create => "CREATE",
                        ContextChangeAction::Update => "UPDATE",
                        ContextChangeAction::Delete => "DELETE",
                        ContextChangeAction::Unchanged => "UNCHANGED",
                        ContextChangeAction::Conflict => "CONFLICT",
                    },
                    change.reason.as_str(),
                )
            };
            lines.push(format!(
                "{label:<14}  {: <12}  {: <40}  {detail}",
                change.kind.as_str(),
                change.uri
            ));
        }
    }

    lines.join("\n")
}
