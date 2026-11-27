// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

mod common;

use std::sync::{Arc, Mutex};

use common::{MockResponse, MockServer, TestEnv};
use openviking_sdk::{
    compute_fingerprint, ApplyOptions, AsyncHttpClient, ContextChangeAction, ErrorKind,
    LiveContext, LiveContextOptions,
};
use serde_json::{json, Value};

#[test]
fn arranger_validates_project_name_root_tool_uri_and_tags() {
    let client = AsyncHttpClient::builder()
        .url("http://127.0.0.1:1933")
        .without_config_file()
        .build_with_env(&TestEnv::default())
        .expect("build client");

    assert!(client.arranger("bad name!", None).is_err());
    assert!(client.arranger("demo", Some("viking://memories")).is_err());

    let mut arranger = client
        .arranger("Search-Team_1", Some("viking://resources/team/"))
        .expect("valid arranger");
    assert_eq!(arranger.name(), "search-team_1");
    assert_eq!(arranger.root_uri(), "viking://resources/team");
    assert_eq!(arranger.owner_tag(), "ov-cac-owner=search-team_1");

    // Path must end with .md and stay inside root_uri
    assert!(arranger
        .live_context(
            "status.txt",
            "viking://agent/tools/srv/ping.json",
            "Ping",
            LiveContextOptions::default(),
        )
        .is_err());
    assert!(arranger
        .live_context(
            "../escape.md",
            "viking://agent/tools/srv/ping.json",
            "Ping",
            LiveContextOptions::default(),
        )
        .is_err());
    // Invalid tool_uri
    assert!(arranger
        .live_context(
            "status.md",
            "viking://resources/not-a-tool.json",
            "Ping",
            LiveContextOptions::default(),
        )
        .is_err());
    // Reserved tag rejected
    assert!(arranger
        .live_context(
            "status.md",
            "viking://agent/tools/srv/ping.json",
            "Ping",
            LiveContextOptions {
                tags: vec!["ov-cac-owner=other".to_owned()],
                ..LiveContextOptions::default()
            },
        )
        .is_err());

    // Valid declaration + deterministic SHA-256 fingerprint
    arranger
        .live_context(
            "ops/status.md",
            "viking://agent/tools/srv/ping.json",
            "Service Status",
            LiveContextOptions {
                description: Some("Live status".to_owned()),
                timeout_seconds: 30,
                arguments: serde_json::from_value(json!({ "b": 2, "a": 1 })).expect("map"),
                tags: vec!["env=prod".to_owned(), "team=core".to_owned()],
            },
        )
        .expect("valid live_context");

    let res: &LiveContext = arranger.resources()[0];
    let fp1 = compute_fingerprint(res).expect("fingerprint 1");
    let fp2 = compute_fingerprint(res).expect("fingerprint 2");
    assert_eq!(fp1.len(), 64);
    assert_eq!(fp1, fp2);
}

#[tokio::test]
async fn arranger_plan_and_apply_cover_create_update_unchanged_prune_and_fallback() {
    // First compute the exact fingerprint for `unchanged.md` so the mock server can advertise it.
    let dummy_client = AsyncHttpClient::builder()
        .url("http://127.0.0.1:1933")
        .without_config_file()
        .build_with_env(&TestEnv::default())
        .expect("dummy client");
    let mut seed_arranger = dummy_client
        .arranger("demo", Some("viking://resources/cac"))
        .expect("seed arranger");
    seed_arranger
        .live_context(
            "unchanged.md",
            "viking://agent/tools/metrics/cpu.json",
            "CPU Metrics",
            LiveContextOptions {
                tags: vec!["env=prod".to_owned()],
                ..LiveContextOptions::default()
            },
        )
        .expect("seed unchanged");
    let unchanged_fp = compute_fingerprint(seed_arranger.resources()[0]).expect("fp");

    let batch_mounts_recorded: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let unmounted_uris: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let batch_mounts_clone = Arc::clone(&batch_mounts_recorded);
    let unmounted_clone = Arc::clone(&unmounted_uris);

    let server = MockServer::start(move |req| match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/api/v1/fs/ls") => MockResponse::json(
            200,
            json!({
                "status": "ok",
                "has_more": false,
                "result": [
                    {
                        "uri": "viking://resources/cac/unchanged.md",
                        "isDir": false,
                        "tags": [
                            "env=prod",
                            "ov-cac-owner=demo",
                            "ov-cac-kind=live_context",
                            format!("ov-cac-spec={unchanged_fp}")
                        ]
                    },
                    {
                        "uri": "viking://resources/cac/outdated.md",
                        "isDir": false,
                        "tags": [
                            "ov-cac-owner=demo",
                            "ov-cac-kind=live_context",
                            "ov-cac-spec=0000"
                        ]
                    },
                    {
                        "uri": "viking://resources/cac/obsolete.md",
                        "isDir": false,
                        "tags": [
                            "ov-cac-owner=demo",
                            "ov-cac-kind=live_context",
                            "ov-cac-spec=1111"
                        ]
                    }
                ]
            }),
        ),
        ("GET", "/api/v1/mcps/mounts") => MockResponse::ok_result(json!({
            "mounts": [
                {
                    "node_uri": "viking://resources/cac/unchanged.md",
                    "tool_uri": "viking://agent/tools/metrics/cpu.json",
                    "title": "CPU Metrics",
                    "description": null,
                    "timeout_seconds": 60,
                    "arguments": {}
                }
            ]
        })),
        ("GET", "/api/v1/fs/stat") => {
            // `created.md` does not exist yet -> return 404 NOT_FOUND
            MockResponse::json(
                404,
                json!({
                    "status": "error",
                    "error": { "code": "NOT_FOUND", "message": "not found" }
                }),
            )
        }
        ("POST", "/api/v1/mcps/batch-mount") => {
            let body = req.json_body();
            batch_mounts_clone
                .lock()
                .expect("lock mounts")
                .push(body.clone());
            MockResponse::ok_result(json!({ "succeeded": 2, "failed": 0, "results": [] }))
        }
        ("POST", "/api/v1/mcps/batch-unmount") => {
            // Simulate older server missing batch-unmount -> fallback to DELETE /api/v1/mcps/mount
            MockResponse::json(
                404,
                json!({
                    "status": "error",
                    "error": { "code": "NOT_FOUND", "message": "endpoint not found" }
                }),
            )
        }
        ("DELETE", "/api/v1/mcps/mount") => {
            let uri = req.query_param("node_uri").unwrap_or_default();
            unmounted_clone.lock().expect("lock unmounts").push(uri);
            MockResponse::ok_result(json!({ "unmounted": true }))
        }
        other => panic!("unexpected arranger request: {other:?}"),
    })
    .await;

    let client = AsyncHttpClient::builder()
        .url(server.base_url())
        .without_config_file()
        .build_with_env(&TestEnv::default())
        .expect("build client");

    let mut arranger = client
        .arranger("demo", Some("viking://resources/cac"))
        .expect("arranger");
    arranger
        .live_context(
            "unchanged.md",
            "viking://agent/tools/metrics/cpu.json",
            "CPU Metrics",
            LiveContextOptions {
                tags: vec!["env=prod".to_owned()],
                ..LiveContextOptions::default()
            },
        )
        .expect("unchanged")
        .live_context(
            "outdated.md",
            "viking://agent/tools/metrics/mem.json",
            "Memory Metrics",
            LiveContextOptions::default(),
        )
        .expect("outdated")
        .live_context(
            "created.md",
            "viking://agent/tools/metrics/disk.json",
            "Disk Metrics",
            LiveContextOptions::default(),
        )
        .expect("created");

    let plan = arranger.plan().await.expect("plan");
    assert!(plan.has_changes());
    assert!(!plan.has_conflicts());

    let actions: Vec<(String, ContextChangeAction)> = plan
        .changes
        .iter()
        .map(|c| (c.uri.clone(), c.action))
        .collect();
    assert_eq!(
        actions,
        vec![
            (
                "viking://resources/cac/created.md".to_owned(),
                ContextChangeAction::Create,
            ),
            (
                "viking://resources/cac/outdated.md".to_owned(),
                ContextChangeAction::Update,
            ),
            (
                "viking://resources/cac/unchanged.md".to_owned(),
                ContextChangeAction::Unchanged,
            ),
            (
                "viking://resources/cac/obsolete.md".to_owned(),
                ContextChangeAction::Delete,
            ),
        ]
    );

    // Dry run does not mutate
    let dry = arranger
        .apply(ApplyOptions {
            prune: true,
            dry_run: true,
            wait: false,
        })
        .await
        .expect("dry run");
    assert!(dry.dry_run);
    assert!(dry.applied.is_empty());
    assert!(batch_mounts_recorded.lock().expect("lock").is_empty());

    // Real apply with prune=true exercises batch_mount + 404 fallback to single unmount
    let applied = arranger
        .apply(ApplyOptions {
            prune: true,
            dry_run: false,
            wait: true,
        })
        .await
        .expect("apply");
    assert_eq!(applied.applied.len(), 3);
    assert_eq!(batch_mounts_recorded.lock().expect("lock").len(), 1);
    assert_eq!(
        *unmounted_uris.lock().expect("lock"),
        vec!["viking://resources/cac/obsolete.md".to_owned()]
    );

    let table = applied.format_table();
    assert!(table.contains("Applied:     3"));
    assert!(table.contains("1 created, 1 updated, 1 deleted, 1 unchanged"));
}

#[tokio::test]
async fn arranger_rejects_apply_when_plan_contains_unmanaged_conflict() {
    let server = MockServer::start(|req| match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/api/v1/fs/ls") => MockResponse::json(
            200,
            json!({ "status": "ok", "has_more": false, "result": [] }),
        ),
        ("GET", "/api/v1/fs/stat") => {
            // Unmanaged node already exists at target URI!
            MockResponse::ok_result(json!({ "uri": "viking://resources/cac/conflict.md" }))
        }
        other => panic!("unexpected path: {other:?}"),
    })
    .await;

    let client = AsyncHttpClient::builder()
        .url(server.base_url())
        .without_config_file()
        .build_with_env(&TestEnv::default())
        .expect("client");

    let mut arranger = client
        .arranger("demo", Some("viking://resources/cac"))
        .expect("arranger");
    arranger
        .live_context(
            "conflict.md",
            "viking://agent/tools/srv/ping.json",
            "Ping",
            LiveContextOptions::default(),
        )
        .expect("declare");

    let err = arranger
        .apply(ApplyOptions::default())
        .await
        .expect_err("conflict must fail precondition");
    assert_eq!(err.kind(), ErrorKind::FailedPrecondition);
    assert!(err.details().contains_key("conflicts"));
}
