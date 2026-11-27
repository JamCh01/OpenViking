// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Multi-tenant account, user, and group administration endpoints (`/api/v1/admin/*`).

use http::Method;
use serde_json::{Map, Value};

use crate::client::AsyncHttpClient;
use crate::error::{JsonObject, Result};
use crate::options::{AdminListAccountsOptions, AdminListUsersOptions, AdminUserInitOptions};
use crate::transport::{encode_path_segment, QueryPairs};

impl AsyncHttpClient {
    /// Creates a tenant account and its initial admin user (`POST /api/v1/admin/accounts`).
    pub async fn admin_create_account(
        &self,
        account_id: &str,
        admin_user_id: &str,
        options: AdminUserInitOptions,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert(
            "account_id".to_owned(),
            Value::String(account_id.to_owned()),
        );
        payload.insert(
            "admin_user_id".to_owned(),
            Value::String(admin_user_id.to_owned()),
        );
        if let Some(seed) = options.seed {
            payload.insert("seed".to_owned(), Value::String(seed));
        }
        if let Some(user_config) = options.user_config {
            payload.insert("user_config".to_owned(), Value::Object(user_config));
        }

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/admin/accounts",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Lists tenant accounts (`GET /api/v1/admin/accounts`).
    pub async fn admin_list_accounts(
        &self,
        options: AdminListAccountsOptions,
    ) -> Result<Vec<Value>> {
        let mut query: QueryPairs = Vec::new();
        if let Some(name) = options.name {
            query.push(("name".to_owned(), name));
        }
        if let Some(limit) = options.limit {
            query.push(("limit".to_owned(), limit.to_string()));
            query.push(("page".to_owned(), options.page.to_string()));
        }
        if let Some(q) = options.query {
            query.push(("query".to_owned(), q));
        }

        self.transport
            .request_result(Method::GET, "/api/v1/admin/accounts", &query, None)
            .await
    }

    /// Retrieves credential-free metadata for one account (`GET /api/v1/admin/accounts/{account_id}`).
    pub async fn admin_get_account(&self, account_id: &str) -> Result<JsonObject> {
        let acc = encode_path_segment(account_id);
        self.transport
            .request_result(
                Method::GET,
                &format!("/api/v1/admin/accounts/{acc}"),
                &[],
                None,
            )
            .await
    }

    /// Deletes a tenant account and all associated users (`DELETE /api/v1/admin/accounts/{account_id}`).
    pub async fn admin_delete_account(&self, account_id: &str) -> Result<JsonObject> {
        let acc = encode_path_segment(account_id);
        self.transport
            .request_result(
                Method::DELETE,
                &format!("/api/v1/admin/accounts/{acc}"),
                &[],
                None,
            )
            .await
    }

    /// Registers a user in `account_id` (`POST /api/v1/admin/accounts/{account_id}/users`).
    pub async fn admin_register_user(
        &self,
        account_id: &str,
        user_id: &str,
        role: &str,
        options: AdminUserInitOptions,
    ) -> Result<JsonObject> {
        let effective_role = if role.is_empty() { "user" } else { role };
        let mut payload = Map::new();
        payload.insert("user_id".to_owned(), Value::String(user_id.to_owned()));
        payload.insert("role".to_owned(), Value::String(effective_role.to_owned()));
        if let Some(seed) = options.seed {
            payload.insert("seed".to_owned(), Value::String(seed));
        }
        if let Some(user_config) = options.user_config {
            payload.insert("user_config".to_owned(), Value::Object(user_config));
        }

        let acc = encode_path_segment(account_id);
        self.transport
            .request_result(
                Method::POST,
                &format!("/api/v1/admin/accounts/{acc}/users"),
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Lists users in `account_id` (`GET /api/v1/admin/accounts/{account_id}/users`).
    pub async fn admin_list_users(
        &self,
        account_id: &str,
        options: AdminListUsersOptions,
    ) -> Result<Vec<Value>> {
        let mut query: QueryPairs = Vec::new();
        if let Some(limit) = options.limit {
            query.push(("limit".to_owned(), limit.to_string()));
            query.push(("page".to_owned(), options.page.to_string()));
        }
        if let Some(name) = options.name {
            query.push(("name".to_owned(), name));
        }
        if let Some(role) = options.role {
            query.push(("role".to_owned(), role));
        }
        if let Some(q) = options.query {
            query.push(("query".to_owned(), q));
        }

        let acc = encode_path_segment(account_id);
        self.transport
            .request_result(
                Method::GET,
                &format!("/api/v1/admin/accounts/{acc}/users"),
                &query,
                None,
            )
            .await
    }

    /// Retrieves credential-free membership info for one user (`GET /api/v1/admin/accounts/{acc}/users/{uid}`).
    pub async fn admin_get_user(&self, account_id: &str, user_id: &str) -> Result<JsonObject> {
        let acc = encode_path_segment(account_id);
        let uid = encode_path_segment(user_id);
        self.transport
            .request_result(
                Method::GET,
                &format!("/api/v1/admin/accounts/{acc}/users/{uid}"),
                &[],
                None,
            )
            .await
    }

    /// Removes a user from `account_id` (`DELETE /api/v1/admin/accounts/{acc}/users/{uid}`).
    pub async fn admin_remove_user(&self, account_id: &str, user_id: &str) -> Result<JsonObject> {
        let acc = encode_path_segment(account_id);
        let uid = encode_path_segment(user_id);
        self.transport
            .request_result(
                Method::DELETE,
                &format!("/api/v1/admin/accounts/{acc}/users/{uid}"),
                &[],
                None,
            )
            .await
    }

    /// Updates a user's role (`PUT /api/v1/admin/accounts/{acc}/users/{uid}/role`).
    pub async fn admin_set_role(
        &self,
        account_id: &str,
        user_id: &str,
        role: &str,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("role".to_owned(), Value::String(role.to_owned()));
        let acc = encode_path_segment(account_id);
        let uid = encode_path_segment(user_id);
        self.transport
            .request_result(
                Method::PUT,
                &format!("/api/v1/admin/accounts/{acc}/users/{uid}/role"),
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Regenerates a user's API key (`POST /api/v1/admin/accounts/{acc}/users/{uid}/key`).
    pub async fn admin_regenerate_key(
        &self,
        account_id: &str,
        user_id: &str,
        seed: Option<&str>,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        if let Some(s) = seed {
            payload.insert("seed".to_owned(), Value::String(s.to_owned()));
        }
        let acc = encode_path_segment(account_id);
        let uid = encode_path_segment(user_id);
        self.transport
            .request_result(
                Method::POST,
                &format!("/api/v1/admin/accounts/{acc}/users/{uid}/key"),
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Creates an account-scoped group (`POST /api/v1/admin/accounts/{acc}/groups`).
    pub async fn admin_create_group(&self, account_id: &str, group_id: &str) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("group_id".to_owned(), Value::String(group_id.to_owned()));
        let acc = encode_path_segment(account_id);
        self.transport
            .request_result(
                Method::POST,
                &format!("/api/v1/admin/accounts/{acc}/groups"),
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Lists account-scoped groups (`GET /api/v1/admin/accounts/{acc}/groups`).
    pub async fn admin_list_groups(&self, account_id: &str) -> Result<Vec<Value>> {
        let acc = encode_path_segment(account_id);
        self.transport
            .request_result(
                Method::GET,
                &format!("/api/v1/admin/accounts/{acc}/groups"),
                &[],
                None,
            )
            .await
    }

    /// Deletes an empty group (`DELETE /api/v1/admin/accounts/{acc}/groups/{gid}`).
    pub async fn admin_delete_group(&self, account_id: &str, group_id: &str) -> Result<JsonObject> {
        let acc = encode_path_segment(account_id);
        let gid = encode_path_segment(group_id);
        self.transport
            .request_result(
                Method::DELETE,
                &format!("/api/v1/admin/accounts/{acc}/groups/{gid}"),
                &[],
                None,
            )
            .await
    }

    /// Lists user IDs belonging to a group (`GET /api/v1/admin/accounts/{acc}/groups/{gid}/members`).
    pub async fn admin_list_group_members(
        &self,
        account_id: &str,
        group_id: &str,
    ) -> Result<JsonObject> {
        let acc = encode_path_segment(account_id);
        let gid = encode_path_segment(group_id);
        self.transport
            .request_result(
                Method::GET,
                &format!("/api/v1/admin/accounts/{acc}/groups/{gid}/members"),
                &[],
                None,
            )
            .await
    }

    /// Adds a user to a group (`PUT /api/v1/admin/accounts/{acc}/groups/{gid}/members`).
    pub async fn admin_add_group_member(
        &self,
        account_id: &str,
        group_id: &str,
        user_id: &str,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert(
            "user_ids".to_owned(),
            Value::Array(vec![Value::String(user_id.to_owned())]),
        );
        let acc = encode_path_segment(account_id);
        let gid = encode_path_segment(group_id);
        self.transport
            .request_result(
                Method::PUT,
                &format!("/api/v1/admin/accounts/{acc}/groups/{gid}/members"),
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Removes a user from a group (`DELETE /api/v1/admin/accounts/{acc}/groups/{gid}/members`).
    pub async fn admin_remove_group_member(
        &self,
        account_id: &str,
        group_id: &str,
        user_id: &str,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert(
            "user_ids".to_owned(),
            Value::Array(vec![Value::String(user_id.to_owned())]),
        );
        let acc = encode_path_segment(account_id);
        let gid = encode_path_segment(group_id);
        self.transport
            .request_result(
                Method::DELETE,
                &format!("/api/v1/admin/accounts/{acc}/groups/{gid}/members"),
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }
}
