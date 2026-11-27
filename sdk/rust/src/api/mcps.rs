// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! MCP server registration, tool invocation, and Live Context mount endpoints (`/api/v1/mcps*`).

use std::collections::BTreeMap;

use http::Method;
use serde_json::{Map, Value};

use crate::client::AsyncHttpClient;
use crate::error::{JsonObject, Result};
use crate::transport::{encode_path_segment, QueryPairs};
use crate::types::McpToolMount;
use crate::uri::VikingUri;

/// Optional parameters for registering or syncing an MCP server.
#[derive(Debug, Clone, Default)]
pub struct McpScopeOptions {
    /// Target tool space URI (e.g. `"viking://~/tools"` or `"viking://agent/tools"`).
    pub target_uri: Option<String>,
    /// Wait for background tool discovery/indexing to finish before returning.
    pub wait: bool,
}

/// Optional parameters for mounting a single MCP tool as a Live Context node.
#[derive(Debug, Clone)]
pub struct MountMcpToolOptions {
    /// Optional human-readable description.
    pub description: Option<String>,
    /// Tool execution timeout in seconds (defaults to 60).
    pub timeout_seconds: i64,
    /// Arguments passed when evaluating the tool.
    pub arguments: JsonObject,
    /// Optional retrieval tags attached to the mounted node.
    pub tags: Option<Vec<String>>,
    /// Wait for initial evaluation/indexing to complete.
    pub wait: bool,
}

impl Default for MountMcpToolOptions {
    fn default() -> Self {
        Self {
            description: None,
            timeout_seconds: 60,
            arguments: JsonObject::new(),
            tags: None,
            wait: false,
        }
    }
}

impl AsyncHttpClient {
    /// Registers a Streamable HTTP MCP server (`POST /api/v1/mcps`).
    pub async fn add_mcp(
        &self,
        name: &str,
        url: &str,
        headers: Option<&BTreeMap<String, String>>,
        options: McpScopeOptions,
    ) -> Result<JsonObject> {
        let mut header_map = Map::new();
        if let Some(hdrs) = headers {
            for (k, v) in hdrs {
                header_map.insert(k.clone(), Value::String(v.clone()));
            }
        }
        let mut payload = Map::new();
        payload.insert("name".to_owned(), Value::String(name.to_owned()));
        payload.insert("url".to_owned(), Value::String(url.to_owned()));
        payload.insert("headers".to_owned(), Value::Object(header_map));
        payload.insert("wait".to_owned(), Value::Bool(options.wait));
        if let Some(target_uri) = &options.target_uri {
            payload.insert(
                "target_uri".to_owned(),
                Value::String(VikingUri::normalize(target_uri)),
            );
        }

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/mcps",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Removes a registered MCP server (`DELETE /api/v1/mcps/{name}`).
    pub async fn remove_mcp(&self, name: &str, target_uri: Option<&str>) -> Result<JsonObject> {
        let mut query: QueryPairs = Vec::new();
        if let Some(uri) = target_uri {
            query.push(("target_uri".to_owned(), VikingUri::normalize(uri)));
        }
        let encoded = encode_path_segment(name);
        self.transport
            .request_result(
                Method::DELETE,
                &format!("/api/v1/mcps/{encoded}"),
                &query,
                None,
            )
            .await
    }

    /// Lists registered MCP servers (`GET /api/v1/mcps`).
    pub async fn list_mcps(&self, target_uri: Option<&str>) -> Result<JsonObject> {
        let mut query: QueryPairs = Vec::new();
        if let Some(uri) = target_uri {
            query.push(("target_uri".to_owned(), VikingUri::normalize(uri)));
        }
        self.transport
            .request_result(Method::GET, "/api/v1/mcps", &query, None)
            .await
    }

    /// Searches registered MCP tools semantically (`POST /api/v1/mcps/find`).
    pub async fn find_mcp_tools(
        &self,
        query: &str,
        limit: usize,
        target_uri: Option<&str>,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("query".to_owned(), Value::String(query.to_owned()));
        payload.insert("limit".to_owned(), Value::Number(limit.into()));
        if let Some(uri) = target_uri {
            payload.insert(
                "target_uri".to_owned(),
                Value::String(VikingUri::normalize(uri)),
            );
        }
        self.transport
            .request_result(
                Method::POST,
                "/api/v1/mcps/find",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Validates connectivity and discovers tools for an MCP server without registering it.
    pub async fn validate_mcp(
        &self,
        name: &str,
        url: &str,
        headers: Option<&BTreeMap<String, String>>,
    ) -> Result<JsonObject> {
        let mut header_map = Map::new();
        if let Some(hdrs) = headers {
            for (k, v) in hdrs {
                header_map.insert(k.clone(), Value::String(v.clone()));
            }
        }
        let mut payload = Map::new();
        payload.insert("name".to_owned(), Value::String(name.to_owned()));
        payload.insert("url".to_owned(), Value::String(url.to_owned()));
        payload.insert("headers".to_owned(), Value::Object(header_map));

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/mcps/validate",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Retrieves one registered MCP server and its discovered tool catalog (`GET /api/v1/mcps/{name}`).
    pub async fn get_mcp(&self, name: &str, target_uri: Option<&str>) -> Result<JsonObject> {
        let mut query: QueryPairs = Vec::new();
        if let Some(uri) = target_uri {
            query.push(("target_uri".to_owned(), VikingUri::normalize(uri)));
        }
        let encoded = encode_path_segment(name);
        self.transport
            .request_result(
                Method::GET,
                &format!("/api/v1/mcps/{encoded}"),
                &query,
                None,
            )
            .await
    }

    /// Refreshes an MCP server's tool catalog (`POST /api/v1/mcps/{name}/sync`).
    pub async fn sync_mcp(&self, name: &str, options: McpScopeOptions) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("wait".to_owned(), Value::Bool(options.wait));
        if let Some(target_uri) = &options.target_uri {
            payload.insert(
                "target_uri".to_owned(),
                Value::String(VikingUri::normalize(target_uri)),
            );
        }
        let encoded = encode_path_segment(name);
        self.transport
            .request_result(
                Method::POST,
                &format!("/api/v1/mcps/{encoded}/sync"),
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Checks live reachability status of a registered MCP server (`GET /api/v1/mcps/status`).
    pub async fn get_mcp_status(&self, name: &str, target_uri: Option<&str>) -> Result<JsonObject> {
        let mut query: QueryPairs = vec![("name".to_owned(), name.to_owned())];
        if let Some(uri) = target_uri {
            query.push(("target_uri".to_owned(), VikingUri::normalize(uri)));
        }
        self.transport
            .request_result(Method::GET, "/api/v1/mcps/status", &query, None)
            .await
    }

    /// Invokes a registered MCP tool through OpenViking (`POST /api/v1/mcps/invoke`).
    pub async fn invoke_mcp_tool(
        &self,
        tool_uri: &str,
        arguments: Option<&JsonObject>,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert(
            "uri".to_owned(),
            Value::String(VikingUri::normalize(tool_uri)),
        );
        payload.insert(
            "arguments".to_owned(),
            Value::Object(arguments.cloned().unwrap_or_default()),
        );
        self.transport
            .request_result(
                Method::POST,
                "/api/v1/mcps/invoke",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Mounts an MCP tool as a Live Context resource node (`POST /api/v1/mcps/mount`).
    pub async fn mount_mcp_tool(
        &self,
        tool_uri: &str,
        node_uri: &str,
        title: &str,
        options: MountMcpToolOptions,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert(
            "tool_uri".to_owned(),
            Value::String(VikingUri::normalize(tool_uri)),
        );
        payload.insert(
            "node_uri".to_owned(),
            Value::String(VikingUri::normalize(node_uri)),
        );
        payload.insert("title".to_owned(), Value::String(title.to_owned()));
        payload.insert(
            "description".to_owned(),
            match options.description {
                Some(desc) => Value::String(desc),
                None => Value::Null,
            },
        );
        payload.insert(
            "timeout_seconds".to_owned(),
            Value::Number(options.timeout_seconds.into()),
        );
        payload.insert("arguments".to_owned(), Value::Object(options.arguments));
        payload.insert("wait".to_owned(), Value::Bool(options.wait));
        if let Some(tags) = options.tags {
            payload.insert(
                "tags".to_owned(),
                Value::Array(tags.into_iter().map(Value::String).collect()),
            );
        }

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/mcps/mount",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Mounts multiple MCP tools as Live Context nodes in order (`POST /api/v1/mcps/batch-mount`).
    pub async fn batch_mount_mcp_tools(
        &self,
        mounts: &[McpToolMount],
        wait: bool,
    ) -> Result<JsonObject> {
        let mut items = Vec::with_capacity(mounts.len());
        for mount in mounts {
            let mut item = Map::new();
            item.insert(
                "tool_uri".to_owned(),
                Value::String(VikingUri::normalize(&mount.tool_uri)),
            );
            item.insert(
                "node_uri".to_owned(),
                Value::String(VikingUri::normalize(&mount.node_uri)),
            );
            item.insert("title".to_owned(), Value::String(mount.title.clone()));
            item.insert(
                "description".to_owned(),
                match &mount.description {
                    Some(desc) => Value::String(desc.clone()),
                    None => Value::Null,
                },
            );
            item.insert(
                "timeout_seconds".to_owned(),
                Value::Number(mount.timeout_seconds.unwrap_or(60).into()),
            );
            item.insert(
                "arguments".to_owned(),
                Value::Object(mount.arguments.clone()),
            );
            if let Some(tags) = &mount.tags {
                item.insert(
                    "tags".to_owned(),
                    Value::Array(tags.iter().cloned().map(Value::String).collect()),
                );
            }
            items.push(Value::Object(item));
        }

        let mut payload = Map::new();
        payload.insert("mounts".to_owned(), Value::Array(items));
        payload.insert("wait".to_owned(), Value::Bool(wait));

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/mcps/batch-mount",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Unmounts one Live Context node (`DELETE /api/v1/mcps/mount`).
    pub async fn unmount_mcp_tool(&self, node_uri: &str, wait: bool) -> Result<JsonObject> {
        let query: QueryPairs = vec![
            ("node_uri".to_owned(), VikingUri::normalize(node_uri)),
            ("wait".to_owned(), wait.to_string()),
        ];
        self.transport
            .request_result(Method::DELETE, "/api/v1/mcps/mount", &query, None)
            .await
    }

    /// Unmounts multiple Live Context nodes in order (`POST /api/v1/mcps/batch-unmount`).
    pub async fn batch_unmount_mcp_tools(
        &self,
        node_uris: &[String],
        wait: bool,
    ) -> Result<JsonObject> {
        let uris: Vec<Value> = node_uris
            .iter()
            .map(|u| Value::String(VikingUri::normalize(u)))
            .collect();
        let mut payload = Map::new();
        payload.insert("node_uris".to_owned(), Value::Array(uris));
        payload.insert("wait".to_owned(), Value::Bool(wait));

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/mcps/batch-unmount",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Lists Live Context mounts backed by a given MCP server (`GET /api/v1/mcps/mounts`).
    pub async fn list_mcp_mounts(
        &self,
        mcp: &str,
        tool: Option<&str>,
        target_uri: Option<&str>,
    ) -> Result<JsonObject> {
        let mut query: QueryPairs = vec![("mcp".to_owned(), mcp.to_owned())];
        if let Some(tool_name) = tool {
            query.push(("tool".to_owned(), tool_name.to_owned()));
        }
        if let Some(uri) = target_uri {
            query.push(("target_uri".to_owned(), VikingUri::normalize(uri)));
        }
        self.transport
            .request_result(Method::GET, "/api/v1/mcps/mounts", &query, None)
            .await
    }
}
