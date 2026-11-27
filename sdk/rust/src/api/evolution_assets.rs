// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Agent Evolution Experience analytics and OpenViking Assets manifest/preflight endpoints.

use http::Method;
use serde_json::{Map, Value};

use crate::client::AsyncHttpClient;
use crate::error::{JsonObject, Result};
use crate::options::{
    compact_request_body, merge_extra, ExperienceOutcomeOptions, ExperienceTrajectoryOptions,
    PreflightAssetOptions, ResolveAssetsOptions,
};
use crate::transport::QueryPairs;
use crate::uri::VikingUri;

impl AsyncHttpClient {
    /// Lists trajectories that consumed an Experience (`GET /api/v1/agent-evolution/experiences/trajectories`).
    pub async fn list_experience_trajectories(
        &self,
        experience_uri: &str,
        options: ExperienceTrajectoryOptions,
    ) -> Result<JsonObject> {
        let mut query: QueryPairs = vec![(
            "experience_uri".to_owned(),
            VikingUri::normalize(experience_uri),
        )];
        if let Some(limit) = options.limit {
            query.push(("limit".to_owned(), limit.to_string()));
        }
        if let Some(offset) = options.offset {
            query.push(("offset".to_owned(), offset.to_string()));
        }
        if let Some(start_date) = options.start_date {
            query.push(("start_date".to_owned(), start_date));
        }
        if let Some(end_date) = options.end_date {
            query.push(("end_date".to_owned(), end_date));
        }

        self.transport
            .request_result(
                Method::GET,
                "/api/v1/agent-evolution/experiences/trajectories",
                &query,
                None,
            )
            .await
    }

    /// Returns outcome distribution metrics for an Experience (`GET /api/v1/agent-evolution/experiences/outcomes`).
    pub async fn get_experience_outcomes(
        &self,
        experience_uri: &str,
        options: ExperienceOutcomeOptions,
    ) -> Result<JsonObject> {
        let mut query: QueryPairs = vec![(
            "experience_uri".to_owned(),
            VikingUri::normalize(experience_uri),
        )];
        if let Some(start_date) = options.start_date {
            query.push(("start_date".to_owned(), start_date));
        }
        if let Some(end_date) = options.end_date {
            query.push(("end_date".to_owned(), end_date));
        }

        self.transport
            .request_result(
                Method::GET,
                "/api/v1/agent-evolution/experiences/outcomes",
                &query,
                None,
            )
            .await
    }

    /// Parses and validates an OpenViking Assets manifest (`POST /api/v1/openviking-assets/resolve`).
    pub async fn resolve_openviking_assets(
        &self,
        manifest_yaml: &str,
        options: ResolveAssetsOptions,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert(
            "manifest_yaml".to_owned(),
            Value::String(manifest_yaml.to_owned()),
        );
        if let Some(catalog_yaml) = options.catalog_yaml {
            payload.insert("catalog_yaml".to_owned(), Value::String(catalog_yaml));
        }
        if let Some(manifest_label) = options.manifest_label {
            payload.insert("manifest_label".to_owned(), Value::String(manifest_label));
        }
        if let Some(catalog_label) = options.catalog_label {
            payload.insert("catalog_label".to_owned(), Value::String(catalog_label));
        }
        merge_extra(&mut payload, &options.extra, &["manifest_yaml"])?;
        compact_request_body(&mut payload);

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/openviking-assets/resolve",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }

    /// Verifies read access to one Git asset (`POST /api/v1/openviking-assets/preflight`).
    pub async fn preflight_openviking_asset(
        &self,
        name: &str,
        repo_url: &str,
        options: PreflightAssetOptions,
    ) -> Result<JsonObject> {
        let mut payload = Map::new();
        payload.insert("name".to_owned(), Value::String(name.to_owned()));
        payload.insert("connector".to_owned(), Value::String("git".to_owned()));
        payload.insert("repo_url".to_owned(), Value::String(repo_url.to_owned()));
        if let Some(branch) = options.branch {
            payload.insert("branch".to_owned(), Value::String(branch));
        }
        if let Some(commit) = options.commit {
            payload.insert("commit".to_owned(), Value::String(commit));
        }
        if let Some(auth_config) = options.auth_config {
            payload.insert("auth_config".to_owned(), serde_json::to_value(auth_config)?);
        }
        merge_extra(
            &mut payload,
            &options.extra,
            &["name", "connector", "repo_url"],
        )?;
        compact_request_body(&mut payload);

        self.transport
            .request_result(
                Method::POST,
                "/api/v1/openviking-assets/preflight",
                &[],
                Some(&Value::Object(payload)),
            )
            .await
    }
}
