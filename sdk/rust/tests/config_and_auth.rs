// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

mod common;

use std::sync::{Arc, Mutex};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use common::{MockResponse, MockServer, TestEnv};
use openviking_sdk::{AsyncHttpClient, ErrorKind, OPENVIKING_CLI_CONFIG_ENV};
use serde_json::json;
use tempfile::tempdir;

#[tokio::test]
async fn explicit_arguments_win_over_env_and_ovcli_file() {
    let file_server =
        MockServer::start(|_| MockResponse::json(200, json!({ "status": "ok" }))).await;
    let env_server =
        MockServer::start(|_| MockResponse::json(200, json!({ "status": "ok" }))).await;
    let explicit_server =
        MockServer::start(|_| MockResponse::json(200, json!({ "status": "ok" }))).await;

    let dir = tempdir().expect("tempdir");
    let conf_path = dir.path().join("ovcli.conf");
    std::fs::write(
        &conf_path,
        serde_json::to_vec(&json!({
            "url": file_server.base_url(),
            "api_key": "file-key",
            "account": "file-account",
            "user": "file-user",
            "actor_peer_id": "file-actor",
            "timeout": 8,
            "profile": true,
            "extra_headers": { "X-Custom": "file", "X-File-Only": "file" }
        }))
        .expect("serialize conf"),
    )
    .expect("write conf");

    let file_env = TestEnv::default().with_var(
        OPENVIKING_CLI_CONFIG_ENV,
        conf_path.to_string_lossy().into_owned(),
    );
    let file_client = AsyncHttpClient::builder()
        .build_with_env(&file_env)
        .expect("build from file");
    file_client.initialize().await.expect("initialize");
    assert!(file_client.health().await);
    file_client.close().await;

    let reqs = file_server.requests();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].header("x-api-key"), Some("file-key"));
    assert_eq!(reqs[0].header("x-openviking-account"), Some("file-account"));
    assert_eq!(reqs[0].header("x-openviking-user"), Some("file-user"));
    assert_eq!(
        reqs[0].header("x-openviking-actor-peer"),
        Some("file-actor")
    );
    assert_eq!(reqs[0].header("x-custom"), Some("file"));
    assert_eq!(reqs[0].header("x-file-only"), Some("file"));
    assert_eq!(reqs[0].query_param("profile").as_deref(), Some("1"));

    let env_override = file_env
        .with_var("OPENVIKING_URL", env_server.base_url())
        .with_var("OPENVIKING_API_KEY", "env-key")
        .with_var("OPENVIKING_ACCOUNT", "env-account")
        .with_var("OPENVIKING_USER", "env-user")
        .with_var("OPENVIKING_ACTOR_PEER_ID", "env-actor")
        .with_var("OPENVIKING_TIMEOUT", "12.5");
    let env_client = AsyncHttpClient::builder()
        .build_with_env(&env_override)
        .expect("build from env");
    assert!(env_client.health().await);

    let env_reqs = env_server.requests();
    assert_eq!(env_reqs.len(), 1);
    assert_eq!(env_reqs[0].header("x-api-key"), Some("env-key"));
    assert_eq!(
        env_reqs[0].header("x-openviking-account"),
        Some("env-account")
    );
    assert_eq!(env_reqs[0].header("x-openviking-user"), Some("env-user"));
    assert_eq!(
        env_reqs[0].header("x-openviking-actor-peer"),
        Some("env-actor")
    );

    let explicit_client = AsyncHttpClient::builder()
        .url(explicit_server.base_url())
        .api_key("explicit-key")
        .account("explicit-account")
        .user("explicit-user")
        .actor_peer_id("explicit-actor")
        .timeout_secs(60.0)
        .profile_enabled(false)
        .extra_headers([("X-Custom", "explicit")])
        .build_with_env(&env_override)
        .expect("build explicit");
    assert!(explicit_client.health().await);

    let exp_reqs = explicit_server.requests();
    assert_eq!(exp_reqs.len(), 1);
    assert_eq!(exp_reqs[0].header("x-api-key"), Some("explicit-key"));
    assert_eq!(
        exp_reqs[0].header("x-openviking-account"),
        Some("explicit-account")
    );
    assert_eq!(
        exp_reqs[0].header("x-openviking-user"),
        Some("explicit-user")
    );
    assert_eq!(
        exp_reqs[0].header("x-openviking-actor-peer"),
        Some("explicit-actor")
    );
    assert_eq!(exp_reqs[0].header("x-custom"), Some("explicit"));
    assert_eq!(exp_reqs[0].header("x-file-only"), None);
    assert_eq!(exp_reqs[0].query_param("profile"), None);
}

#[test]
fn url_is_required_when_missing_everywhere() {
    let empty_env = TestEnv::default();
    let err = AsyncHttpClient::builder()
        .without_config_file()
        .build_with_env(&empty_env)
        .expect_err("missing url must fail");
    assert_eq!(err.kind(), ErrorKind::Config);
    assert!(err.message().contains("url is required"));
}

#[test]
fn invalid_ovcli_config_fields_and_conflicting_actor_peer_are_rejected() {
    let cases = [
        (
            json!({ "url": "http://localhost:1933", "timeout": "fast" }),
            "ovcli.timeout",
        ),
        (
            json!({ "url": "http://localhost:1933", "ur": "http://localhost:1933" }),
            "ovcli.url",
        ),
        (
            json!({ "url": "http://localhost:1933", "upload": { "unknown": "value" } }),
            "ovcli.upload.unknown",
        ),
        (
            json!({
                "url": "http://localhost:1933",
                "actor_peer_id": "actor-a",
                "agent_id": "legacy-agent"
            }),
            "actor_peer_id cannot be used with agent_id",
        ),
    ];

    for (payload, expected_substr) in cases {
        let dir = tempdir().expect("tempdir");
        let conf_path = dir.path().join("ovcli.conf");
        std::fs::write(&conf_path, serde_json::to_vec(&payload).expect("json")).expect("write");
        let env = TestEnv::default().with_var(
            OPENVIKING_CLI_CONFIG_ENV,
            conf_path.to_string_lossy().into_owned(),
        );
        let err = AsyncHttpClient::builder()
            .build_with_env(&env)
            .expect_err("invalid ovcli.conf must error");
        assert_eq!(err.kind(), ErrorKind::Config);
        assert!(
            err.message().contains(expected_substr),
            "expected '{expected_substr}' in '{}'",
            err.message()
        );
    }

    let explicit_conflict = AsyncHttpClient::builder()
        .url("http://localhost:1933")
        .actor_peer_id("actor-a")
        .agent_id("legacy-agent")
        .without_config_file()
        .build_with_env(&TestEnv::default())
        .expect_err("explicit actor_peer_id + agent_id conflict");
    assert!(explicit_conflict
        .message()
        .contains("actor_peer_id cannot be used with agent_id"));
}

#[tokio::test]
async fn gateway_token_retries_only_after_marked_same_origin_challenge() {
    let gateway_server = MockServer::start(|req| {
        if req.header("x-gateway-token") == Some("configured-token") {
            MockResponse::json(200, json!({ "status": "ok" }))
        } else {
            MockResponse::json(401, json!({ "detail": "X-Gateway-Token header required" }))
                .with_header("X-VikingBot-Gateway", "true")
        }
    })
    .await;

    let other_server = MockServer::start(|_| {
        MockResponse::json(401, json!({ "detail": "X-Gateway-Token header required" }))
            .with_header("X-VikingBot-Gateway", "true")
    })
    .await;

    // Case 1: configured gateway on same origin retries with configured-token.
    let dir = tempdir().expect("tempdir");
    let conf_path = dir.path().join("ovcli.conf");
    std::fs::write(
        &conf_path,
        serde_json::to_vec(&json!({
            "url": format!("{}/", gateway_server.base_url()),
            "gateway_token": "configured-token"
        }))
        .expect("json"),
    )
    .expect("write");
    let env = TestEnv::default().with_var(
        OPENVIKING_CLI_CONFIG_ENV,
        conf_path.to_string_lossy().into_owned(),
    );

    let client = AsyncHttpClient::builder()
        .build_with_env(&env)
        .expect("build gateway client");
    assert!(client.health().await);
    let tokens: Vec<Option<String>> = gateway_server
        .requests()
        .into_iter()
        .map(|r| r.header("x-gateway-token").map(str::to_owned))
        .collect();
    assert_eq!(tokens, vec![None, Some("configured-token".to_owned())]);

    // Case 2: explicit x-gateway-token header does not trigger fallback retry.
    gateway_server.clear_requests();
    let manual_client = AsyncHttpClient::builder()
        .extra_headers([("x-gateway-token", "manual-token")])
        .build_with_env(&env)
        .expect("build manual header client");
    assert!(!manual_client.health().await);
    let manual_tokens: Vec<Option<String>> = gateway_server
        .requests()
        .into_iter()
        .map(|r| r.header("x-gateway-token").map(str::to_owned))
        .collect();
    assert_eq!(manual_tokens, vec![Some("manual-token".to_owned())]);

    // Case 3: explicit different server URL must not leak ovcli.conf gateway_token.
    let other_client = AsyncHttpClient::builder()
        .url(other_server.base_url())
        .build_with_env(&env)
        .expect("build other-server client");
    assert!(!other_client.health().await);
    let other_tokens: Vec<Option<String>> = other_server
        .requests()
        .into_iter()
        .map(|r| r.header("x-gateway-token").map(str::to_owned))
        .collect();
    assert_eq!(other_tokens, vec![None]);

    // Case 4: unmarked 401 does not retry with gateway token.
    let unmarked_server = MockServer::start(|_| {
        MockResponse::json(
            401,
            json!({
                "status": "error",
                "error": { "code": "UNAUTHENTICATED", "message": "Invalid key" }
            }),
        )
    })
    .await;
    let unmarked_conf = dir.path().join("unmarked.conf");
    std::fs::write(
        &unmarked_conf,
        serde_json::to_vec(&json!({
            "url": unmarked_server.base_url(),
            "gateway_token": "configured-token"
        }))
        .expect("json"),
    )
    .expect("write");
    let unmarked_env = TestEnv::default().with_var(
        OPENVIKING_CLI_CONFIG_ENV,
        unmarked_conf.to_string_lossy().into_owned(),
    );
    let unmarked_client = AsyncHttpClient::builder()
        .build_with_env(&unmarked_env)
        .expect("build unmarked client");
    assert!(!unmarked_client.health().await);
    assert_eq!(unmarked_server.requests().len(), 1);
    assert_eq!(
        unmarked_server.requests()[0].header("x-gateway-token"),
        None
    );
}

#[tokio::test]
async fn ldap_oidc_and_request_hooks_behave_like_python_sdk() {
    let server = MockServer::start(|_| MockResponse::json(200, json!({ "status": "ok" }))).await;

    let dir = tempdir().expect("tempdir");
    let conf_path = dir.path().join("ovcli.conf");
    std::fs::write(
        &conf_path,
        serde_json::to_vec(&json!({
            "url": server.base_url(),
            "auth_mode": "ldap",
            "ldap_username": "file-user",
            "ldap_password": "file-pass",
            "extra_headers": { "X-Custom": "routing" }
        }))
        .expect("json"),
    )
    .expect("write");

    let env = TestEnv::default().with_var(
        OPENVIKING_CLI_CONFIG_ENV,
        conf_path.to_string_lossy().into_owned(),
    );
    let file_client = AsyncHttpClient::builder()
        .build_with_env(&env)
        .expect("ldap file client");
    assert!(file_client.health().await);
    let last = server.requests().pop().expect("request");
    assert_eq!(
        last.header("authorization"),
        Some("Basic ZmlsZS11c2VyOmZpbGUtcGFzcw==")
    );
    assert_eq!(last.header("x-custom"), Some("routing"));
    assert_eq!(last.header("x-api-key"), None);

    // Env override for LDAP
    let env_ldap = env
        .with_var("OPENVIKING_AUTH_MODE", "ldap")
        .with_var("OPENVIKING_USERNAME", "env-user")
        .with_var("OPENVIKING_PASSWORD", "env-pass");
    let env_client = AsyncHttpClient::builder()
        .build_with_env(&env_ldap)
        .expect("ldap env client");
    assert!(env_client.health().await);
    let last = server.requests().pop().expect("request");
    assert_eq!(
        last.header("authorization"),
        Some("Basic ZW52LXVzZXI6ZW52LXBhc3M=")
    );

    // Explicit UTF-8 LDAP + API key coexistence
    let utf8_client = AsyncHttpClient::builder()
        .url(server.base_url())
        .auth_mode("ldap")
        .ldap_username("alice")
        .ldap_password("päss:w0rd!")
        .api_key("test-key")
        .without_config_file()
        .build_with_env(&TestEnv::default())
        .expect("utf8 ldap client");
    assert!(utf8_client.health().await);
    let last = server.requests().pop().expect("request");
    let auth_hdr = last.header("authorization").expect("authorization");
    let encoded = auth_hdr.strip_prefix("Basic ").expect("Basic scheme");
    let decoded = BASE64_STANDARD.decode(encoded).expect("decode base64");
    assert_eq!(decoded, "alice:päss:w0rd!".as_bytes());
    assert_eq!(last.header("x-api-key"), Some("test-key"));

    // Incomplete LDAP does not emit Authorization
    let incomplete_client = AsyncHttpClient::builder()
        .url(server.base_url())
        .auth_mode("ldap")
        .ldap_username("alice")
        .api_key("test-key")
        .without_config_file()
        .build_with_env(&TestEnv::default())
        .expect("incomplete ldap");
    assert!(incomplete_client.health().await);
    let last = server.requests().pop().expect("request");
    assert_eq!(last.header("authorization"), None);
    assert_eq!(last.header("x-api-key"), Some("test-key"));

    // OIDC Bearer + request_hook + JWT api_key fallback
    let hook_paths: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let hook_paths_clone = Arc::clone(&hook_paths);
    let oidc_client = AsyncHttpClient::builder()
        .url(server.base_url())
        .auth_mode("oidc")
        .oidc_token("header.payload.signature")
        .without_config_file()
        .request_hook(move |_method, url, headers| {
            hook_paths_clone
                .lock()
                .expect("lock hook paths")
                .push(url.path().to_owned());
            headers.insert("x-trace-id", http::HeaderValue::from_static("sdk-trace"));
        })
        .build_with_env(&TestEnv::default())
        .expect("oidc client");
    assert!(oidc_client.health().await);
    let last = server.requests().pop().expect("request");
    assert_eq!(
        last.header("authorization"),
        Some("Bearer header.payload.signature")
    );
    assert_eq!(last.header("x-trace-id"), Some("sdk-trace"));
    assert_eq!(
        *hook_paths.lock().expect("lock"),
        vec!["/health".to_owned()]
    );
}
