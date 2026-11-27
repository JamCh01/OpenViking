// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

mod common;

use std::collections::BTreeMap;

use common::{MockResponse, MockServer, TestEnv};
use openviking_sdk::{get_actor_peer_id, use_actor_peer, AsyncHttpClient, ErrorKind, FindOptions};
use serde_json::json;

#[tokio::test]
async fn async_http_client_scopes_actor_peer_without_overriding_tenant() {
    let server = MockServer::start(|_| MockResponse::ok_result(json!({}))).await;

    let client = AsyncHttpClient::builder()
        .url(server.base_url())
        .account("credential-account")
        .user("credential-user")
        .actor_peer_id("configured-assistant")
        .without_config_file()
        .build_with_env(&TestEnv::default())
        .expect("build client");
    assert!(client.supports_request_actor_peer());

    let client_a = client.clone();
    let task_a = tokio::spawn(async move {
        use_actor_peer(Some("assistant-a"), async move {
            client_a
                .find("memory for assistant-a", FindOptions::default())
                .await
        })
        .await
        .expect("task_a outer")
        .expect("task_a find");
    });

    let client_b = client.clone();
    let task_b = tokio::spawn(async move {
        use_actor_peer(Some("assistant-b"), async move {
            client_b
                .find("memory for assistant-b", FindOptions::default())
                .await
        })
        .await
        .expect("task_b outer")
        .expect("task_b find");
    });

    let (res_a, res_b) = tokio::join!(task_a, task_b);
    res_a.expect("join a");
    res_b.expect("join b");

    client
        .find("no actor peer", FindOptions::default())
        .await
        .expect("default find");

    let reqs = server.requests();
    assert_eq!(reqs.len(), 3);

    let mut by_actor: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for req in &reqs[..2] {
        let actor = req
            .header("x-openviking-actor-peer")
            .expect("actor header")
            .to_owned();
        let mut captured = BTreeMap::new();
        for name in [
            "x-openviking-account",
            "x-openviking-user",
            "x-openviking-actor-peer",
        ] {
            if let Some(val) = req.header(name) {
                captured.insert(name.to_owned(), val.to_owned());
            }
        }
        by_actor.insert(actor, captured);
    }

    assert_eq!(
        by_actor
            .get("assistant-a")
            .and_then(|m| m.get("x-openviking-account"))
            .map(String::as_str),
        Some("credential-account")
    );
    assert_eq!(
        by_actor
            .get("assistant-a")
            .and_then(|m| m.get("x-openviking-user"))
            .map(String::as_str),
        Some("credential-user")
    );
    assert_eq!(
        by_actor
            .get("assistant-b")
            .and_then(|m| m.get("x-openviking-actor-peer"))
            .map(String::as_str),
        Some("assistant-b")
    );

    assert_eq!(
        reqs[2].header("x-openviking-account"),
        Some("credential-account")
    );
    assert_eq!(reqs[2].header("x-openviking-user"), Some("credential-user"));
    assert_eq!(
        reqs[2].header("x-openviking-actor-peer"),
        Some("configured-assistant")
    );
}

#[tokio::test]
async fn actor_peer_scope_is_nested_cleared_restored_and_validated() {
    assert_eq!(get_actor_peer_id(), None);

    use_actor_peer(Some(" outer "), async {
        assert_eq!(get_actor_peer_id().as_deref(), Some("outer"));

        use_actor_peer(Some("inner"), async {
            assert_eq!(get_actor_peer_id().as_deref(), Some("inner"));
        })
        .await
        .expect("inner scope");

        assert_eq!(get_actor_peer_id().as_deref(), Some("outer"));

        use_actor_peer(None, async {
            assert_eq!(get_actor_peer_id(), None);
        })
        .await
        .expect("clear scope");

        assert_eq!(get_actor_peer_id().as_deref(), Some("outer"));
    })
    .await
    .expect("outer scope");

    assert_eq!(get_actor_peer_id(), None);

    let err = use_actor_peer(Some("   "), async { 42 })
        .await
        .expect_err("whitespace-only actor_peer_id must fail");
    assert_eq!(err.kind(), ErrorKind::InvalidArgument);
    assert!(err.message().contains("actor_peer_id must not be empty"));
    assert_eq!(get_actor_peer_id(), None);
}
