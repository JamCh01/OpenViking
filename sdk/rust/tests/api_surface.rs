// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

mod common;

use common::{MockResponse, MockServer, TestEnv};
use openviking_sdk::{
    AclEntry, AclSpec, AddResourceOptions, AddSkillOptions, AdminUserInitOptions, AsyncHttpClient,
    BatchAddMessagesOptions, BatchWriteOperation, BatchWriteOptions, CommitSessionOptions,
    CompileOptions, CompileRequest, ContextPart, CreateSessionOptions, ErrorKind,
    ExperienceOutcomeOptions, ExperienceTrajectoryOptions, FindOptions, GlobOptions, GrepOptions,
    ImageInput, ImagePart, LevelFilter, ListTasksOptions, LsOptions, McpScopeOptions, Message,
    MountMcpToolOptions, Nullable, ObserverFormat, PreflightAssetOptions, ProcessingMode,
    RemoveOptions, ResolveAssetsOptions, RuleCompileOptions, RuleCompileSource, RuleSearchOptions,
    SearchContextOptions, SearchOptions, SetTagsOptions, SkillSource, SnapshotCommitOptions,
    SnapshotDiffPathsOptions, SnapshotLogOptions, SnapshotRestoreOptions, SnapshotShowResult,
    TagMode, TargetUri, TextPart, ToolPart, TreeOptions, UpdateSessionConfigOptions,
    UpdateWatchOptions, WatchRef, WriteOptions,
};
use serde_json::{json, Map, Value};
use tempfile::tempdir;

#[tokio::test]
async fn resources_connector_validation_and_legacy_default_omission() {
    let server = MockServer::start(|_| {
        MockResponse::ok_result(json!({ "root_uri": "viking://resources/feishu" }))
    })
    .await;

    let client = AsyncHttpClient::builder()
        .url(server.base_url())
        .without_config_file()
        .build_with_env(&TestEnv::default())
        .expect("client");

    // Connector requires exact `to`
    let missing_to = client
        .add_resource(
            "space:home",
            AddResourceOptions {
                add_type: Some("feishu".to_owned()),
                ..AddResourceOptions::default()
            },
        )
        .await
        .expect_err("add_type without to must fail");
    assert_eq!(missing_to.kind(), ErrorKind::InvalidArgument);
    assert!(missing_to.message().contains("exact 'to'"));

    // Connector forbids `parent`
    let with_parent = client
        .add_resource(
            "space:home",
            AddResourceOptions {
                to: Some("viking://resources/feishu".to_owned()),
                parent: Some("viking://resources/imports".to_owned()),
                add_type: Some("feishu".to_owned()),
                ..AddResourceOptions::default()
            },
        )
        .await
        .expect_err("add_type with parent must fail");
    assert_eq!(with_parent.kind(), ErrorKind::InvalidArgument);
    assert!(with_parent.message().contains("'parent'"));

    assert!(server.requests().is_empty());

    // Valid connector trims `add_type` and omits default `processing_mode`
    let res = client
        .add_resource(
            "space:home",
            AddResourceOptions {
                to: Some("viking://resources/feishu".to_owned()),
                add_type: Some(" feishu ".to_owned()),
                ..AddResourceOptions::default()
            },
        )
        .await
        .expect("connector add_resource");
    assert_eq!(
        res.get("root_uri"),
        Some(&json!("viking://resources/feishu"))
    );

    let reqs = server.requests();
    assert_eq!(reqs.len(), 1);
    assert_eq!(
        reqs[0].json_body(),
        json!({
            "path": "space:home",
            "to": "viking://resources/feishu",
            "add_type": "feishu",
            "wait": false
        })
    );

    // Remote URL omits default `processing_mode`
    server.clear_requests();
    client
        .add_resource("https://example.com/demo.md", AddResourceOptions::default())
        .await
        .expect("url add_resource");
    let body = server.requests()[0].json_body();
    assert!(body.get("processing_mode").is_none());
}

#[tokio::test]
async fn fs_pagination_record_id_passthrough_write_batch_write_and_tags() {
    let record_id = "0123456789abcdef0123456789abcdef";

    let server = MockServer::start(|req| match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/api/v1/fs/ls") => MockResponse::json(
            200,
            json!({
                "status": "ok",
                "result": [{ "name": "docs" }],
                "has_more": true
            }),
        ),
        ("GET", "/api/v1/fs/tree") => MockResponse::json(
            200,
            json!({
                "status": "ok",
                "result": [{ "name": "docs" }]
            }),
        ),
        ("GET", "/api/v1/fs/stat") => {
            MockResponse::ok_result(json!({ "uri": req.query_param("uri") }))
        }
        ("GET", "/api/v1/content/read") => {
            if req.query_param("raw").as_deref() == Some("true") {
                MockResponse::ok_result(json!("raw body"))
            } else {
                MockResponse::ok_result(json!("file body"))
            }
        }
        ("GET", "/api/v1/content/abstract") => MockResponse::ok_result(json!("abstract body")),
        ("DELETE", "/api/v1/fs") => MockResponse::ok_result(json!(null)),
        ("POST", "/api/v1/content/write") => {
            let body = req.json_body();
            if body.get("resolve_conflicts") == Some(&json!(true)) {
                return MockResponse::json(
                    409,
                    json!({
                        "status": "error",
                        "error": { "code": "CONFLICT", "message": "revision changed" }
                    }),
                );
            }
            MockResponse::ok_result(json!({ "uri": body["uri"] }))
        }
        ("POST", "/api/v1/content/batch-write") => MockResponse::ok_result(json!({ "written": 1 })),
        ("POST", "/api/v1/fs/attrs/set_tags") => MockResponse::ok_result(json!({ "tags": [] })),
        other => panic!("unexpected fs path: {other:?}"),
    })
    .await;

    let client = AsyncHttpClient::builder()
        .url(server.base_url())
        .without_config_file()
        .build_with_env(&TestEnv::default())
        .expect("client");

    let ls_page = client
        .ls_page(
            "/resources/",
            LsOptions {
                simple: true,
                recursive: true,
                output: "agent".to_owned(),
                abs_limit: 32,
                show_all_hidden: true,
                node_limit: 44,
                offset: 5,
                limit: Some(7),
                sort_by: Some("mtime".to_owned()),
                sort_order: "desc".to_owned(),
                include_abstract: Some(false),
                include_overview: Some(true),
                overview_limit: 512,
                ..LsOptions::default()
            },
        )
        .await
        .expect("ls_page");
    assert!(ls_page.has_more);
    assert_eq!(ls_page.result, vec![json!({ "name": "docs" })]);

    let tree_page = client
        .tree_page(
            "viking://resources/",
            TreeOptions {
                level_limit: 2,
                offset: 4,
                limit: Some(6),
                directories_only: true,
                ..TreeOptions::default()
            },
        )
        .await
        .expect("tree_page");
    assert!(!tree_page.has_more);

    // 32-hex record_id passthrough ONLY on stat/read/read_raw, NOT rm!
    server.clear_requests();
    client.stat(record_id).await.expect("stat record_id");
    assert_eq!(
        client.read(record_id, 0, -1).await.expect("read record_id"),
        "file body"
    );
    assert_eq!(
        client
            .read_raw(record_id, 0, -1)
            .await
            .expect("read_raw record_id"),
        "raw body"
    );
    assert_eq!(
        client
            .r#abstract("/resources/doc.md")
            .await
            .expect("abstract"),
        "abstract body"
    );
    client
        .rm(record_id, RemoveOptions::default())
        .await
        .expect("rm record_id");

    let fs_reqs = server.requests();
    assert_eq!(fs_reqs[0].query_param("uri").as_deref(), Some(record_id));
    assert_eq!(fs_reqs[1].query_param("uri").as_deref(), Some(record_id));
    assert_eq!(fs_reqs[2].query_param("uri").as_deref(), Some(record_id));
    assert_eq!(
        fs_reqs[4].query_param("uri").as_deref(),
        Some("viking://0123456789abcdef0123456789abcdef")
    );

    // `write` preserves empty string & omits legacy defaults; conflict propagates
    server.clear_requests();
    client
        .write("viking://resources/demo.md", "", WriteOptions::default())
        .await
        .expect("write empty");
    let conflict = client
        .write(
            "viking://resources/demo.md",
            "resolved",
            WriteOptions {
                use_snapshot: Some(true),
                resolve_conflicts: Some(true),
                processing_mode: Some(ProcessingMode::VectorsOnly),
                ..WriteOptions::default()
            },
        )
        .await
        .expect_err("expected conflict");
    assert_eq!(conflict.kind(), ErrorKind::Conflict);
    let write_reqs = server.requests();
    assert_eq!(write_reqs[0].json_body()["content"], json!(""));
    assert!(write_reqs[0].json_body().get("processing_mode").is_none());
    assert_eq!(
        write_reqs[1].json_body()["processing_mode"],
        json!("vectors_only")
    );

    // `batch_write` + `set_tags(clear)`
    server.clear_requests();
    client
        .batch_write(
            "viking://resources/wiki",
            &[BatchWriteOperation {
                uri: "/resources/wiki/a.md".to_owned(),
                content: Some("new".to_owned()),
                expected_sha256: Some("a".repeat(64)),
                resolve_conflicts: Some(true),
                tags: Some(vec!["team=search".to_owned()]),
                tag_mode: Some(TagMode::Append),
                acl: Some(AclSpec {
                    acl_mode: Some("restricted".to_owned()),
                    entries: Some(Vec::new()),
                }),
                ..BatchWriteOperation::default()
            }],
            BatchWriteOptions {
                processing_mode: Some(ProcessingMode::VectorsOnly),
                skip_conflicts: Some(true),
                use_snapshot: Some(true),
                ..BatchWriteOptions::default()
            },
        )
        .await
        .expect("batch_write");

    client
        .set_tags(
            "viking://resources/demo.md",
            None,
            SetTagsOptions {
                mode: TagMode::Clear,
                ..SetTagsOptions::default()
            },
        )
        .await
        .expect("set_tags clear");

    let bw_reqs = server.requests();
    assert_eq!(
        bw_reqs[0].json_body()["operations"][0]["uri"],
        json!("viking://resources/wiki/a.md")
    );
    assert_eq!(
        bw_reqs[1].json_body(),
        json!({
            "uri": "viking://resources/demo.md",
            "mode": "clear",
            "recursive": false
        })
    );
}

#[tokio::test]
async fn retrieval_find_search_search_context_grep_and_glob() {
    let server = MockServer::start(|req| match req.path.as_str() {
        "/api/v1/search/find" => MockResponse::ok_result(json!({ "resources": [] })),
        "/api/v1/search/search" => {
            let body = req.json_body();
            if body.get("mode") == Some(&json!("context")) {
                MockResponse::ok_result(json!({
                    "rendered": "assembled context",
                    "digest": "sha256:1234",
                    "entries": [
                        { "uri": "viking://resources/doc.md", "category": "resource", "detail": "overview", "score": 0.91 }
                    ],
                    "stats": { "considered": 5 }
                }))
            } else {
                MockResponse::ok_result(json!({ "resources": [] }))
            }
        }
        "/api/v1/search/grep" => MockResponse::ok_result(json!({ "matches": [] })),
        "/api/v1/search/glob" => MockResponse::ok_result(json!({ "uris": [] })),
        other => panic!("unexpected retrieval path: {other}"),
    })
    .await;

    let client = AsyncHttpClient::builder()
        .url(server.base_url())
        .without_config_file()
        .build_with_env(&TestEnv::default())
        .expect("client");

    client
        .find(
            "sample",
            FindOptions {
                target_uri: Some(TargetUri::Multiple(vec![
                    "/resources/demo".to_owned(),
                    "viking://resources/kept".to_owned(),
                ])),
                node_limit: Some(9),
                level: Some(LevelFilter::Single(0)),
                read_content: Some(true),
                image: Some(ImageInput::Bytes {
                    data: b"\x89PNG\r\n\x1a\n".to_vec(),
                    file_name: Some("query.png".to_owned()),
                }),
                ..FindOptions::default()
            },
        )
        .await
        .expect("find");

    client
        .search(
            "sample",
            SearchOptions {
                session_id: Some("thread-123".to_owned()),
                search_type: Some("keywords".to_owned()),
                ..SearchOptions::default()
            },
        )
        .await
        .expect("search");

    let ctx = client
        .search_context(
            "assemble context",
            SearchContextOptions {
                session_id: Some("thread-123".to_owned()),
                image: Some(ImageInput::from("viking://resources/query.png")),
                max_tokens: Some(2048),
                ..SearchContextOptions::default()
            },
        )
        .await
        .expect("search_context");
    assert_eq!(ctx.rendered.as_deref(), Some("assembled context"));
    assert_eq!(ctx.entries.len(), 1);

    client
        .grep(
            TargetUri::Multiple(vec![
                "/resources/demo".to_owned(),
                "viking://~/resources".to_owned(),
            ]),
            "Sample",
            GrepOptions {
                case_insensitive: true,
                node_limit: 12,
                exclude_uri: Some("/resources/demo/tmp".to_owned()),
                level_limit: Some(1),
                ..GrepOptions::default()
            },
        )
        .await
        .expect("grep");

    client
        .glob("*.md", GlobOptions::default())
        .await
        .expect("glob");

    let reqs = server.requests();
    assert_eq!(
        reqs[0].json_body()["target_uri"],
        json!(["viking://resources/demo", "viking://resources/kept"])
    );
    assert_eq!(reqs[0].json_body()["level"], json!(0));
    assert!(reqs[0].json_body()["image_url"]
        .as_str()
        .expect("image_url")
        .starts_with("data:image/png;base64,"));
    assert_eq!(reqs[1].json_body()["session_id"], json!("thread-123"));
    assert!(reqs[1].json_body().get("level").is_none());
    assert_eq!(reqs[2].json_body()["mode"], json!("context"));
    assert_eq!(
        reqs[2].json_body()["image_url"],
        json!("viking://resources/query.png")
    );
    assert_eq!(
        reqs[3].json_body(),
        json!({
            "uri": ["viking://resources/demo", "viking://~/resources"],
            "pattern": "Sample",
            "case_insensitive": true,
            "node_limit": 12,
            "exclude_uri": "viking://resources/demo/tmp",
            "level_limit": 1
        })
    );
}

#[tokio::test]
async fn sessions_message_normalization_escaped_ids_nullable_policy_and_handle() {
    let server = MockServer::start(|req| match (req.method.as_str(), req.path.as_str()) {
        ("POST", "/api/v1/sessions") => MockResponse::ok_result(json!({ "status": "ok" })),
        ("PATCH", "/api/v1/sessions/tagged-session/config") => {
            MockResponse::ok_result(json!({ "status": "ok" }))
        }
        ("POST", "/api/v1/sessions/tagged-session/commit") => {
            MockResponse::ok_result(json!({ "task_id": "t1" }))
        }
        ("POST", "/api/v1/sessions/demo-session/messages") => {
            MockResponse::ok_result(json!({ "message_id": "m1" }))
        }
        ("POST", "/api/v1/sessions/thread%23message/messages/batch") => {
            MockResponse::ok_result(json!({ "added": 1 }))
        }
        ("GET", "/api/v1/sessions/missing-session") => MockResponse::json(
            404,
            json!({
                "status": "error",
                "error": { "code": "NOT_FOUND", "message": "Session does not exist" }
            }),
        ),
        ("GET", "/api/v1/sessions/thread-1") => {
            MockResponse::ok_result(json!({ "session_id": "thread-1" }))
        }
        ("POST", "/api/v1/sessions/thread-1/commit") => {
            MockResponse::ok_result(json!({ "task_id": "task-1" }))
        }
        ("GET", "/api/v1/sessions/thread-1/context") => {
            MockResponse::ok_result(json!({ "messages": [{ "role": "user" }] }))
        }
        ("GET", "/api/v1/sessions/thread-1/archives/arc-1") => {
            MockResponse::ok_result(json!({ "archive_id": "arc-1" }))
        }
        ("DELETE", "/api/v1/sessions/thread-1") => MockResponse::ok_result(json!(null)),
        other => panic!("unexpected session path: {other:?}"),
    })
    .await;

    let client = AsyncHttpClient::builder()
        .url(server.base_url())
        .without_config_file()
        .build_with_env(&TestEnv::default())
        .expect("client");

    // Structured MessagePart normalization
    let mut tool_part = ToolPart::new("call-1", "search");
    tool_part.tool_input =
        Some(serde_json::from_value(json!({ "query": "OpenViking" })).expect("map"));
    tool_part.tool_status = "completed".to_owned();
    tool_part.duration_ms = Some(12.5);
    tool_part.prompt_tokens = Some(42);
    tool_part.completion_tokens = Some(7);
    tool_part.tool_output_ref = "viking://sessions/demo-session/tool-results/call-1".to_owned();
    tool_part.tool_output_truncated = true;
    tool_part.tool_output_original_chars = Some(1024);
    tool_part.tool_output_mime_type = "application/json".to_owned();
    tool_part.tool_output_source_limit = Some(512);

    let msg = Message::with_parts(
        "assistant",
        vec![
            TextPart::new("checking").into(),
            ContextPart::new("viking://resources/guide.md", "resource", "Guide").into(),
            ImagePart::new("https://example.com/image.png")
                .with_detail("high")
                .into(),
            openviking_sdk::MessagePart::from(tool_part),
        ],
    );
    client
        .add_message("demo-session", msg)
        .await
        .expect("add_message parts");

    // URL-escaped session_id (`thread#message` -> `thread%23message`)
    client
        .batch_add_messages(
            "thread#message",
            &[Message::text("user", "hello").peer_id("explicit-user")],
            BatchAddMessagesOptions::default(),
        )
        .await
        .expect("batch_add_messages escaped");

    // Nullable::Null auto_commit_policy & empty event_tags
    let extract_cfg: Map<String, Value> =
        serde_json::from_value(json!({ "events": { "tags": ["team=search"] } })).expect("map");
    client
        .create_session(CreateSessionOptions {
            session_id: Some("disabled-session".to_owned()),
            auto_commit_policy: Nullable::Null,
            memory_extraction_config: Some(extract_cfg),
            ..CreateSessionOptions::default()
        })
        .await
        .expect("create_session null policy");

    client
        .update_session_config(
            "tagged-session",
            UpdateSessionConfigOptions {
                auto_commit_policy: Nullable::Null,
                ..UpdateSessionConfigOptions::default()
            },
        )
        .await
        .expect("update_session_config null policy");

    client
        .commit_session(
            "tagged-session",
            CommitSessionOptions {
                event_tags: Some(Vec::new()),
                ..CommitSessionOptions::default()
            },
        )
        .await
        .expect("commit_session empty event_tags");

    // `session_exists` returns false on 404
    assert!(!client
        .session_exists("missing-session")
        .await
        .expect("session_exists"));

    // `session_checked` + `SessionHandle`
    let handle = client
        .session_checked("thread-1")
        .await
        .expect("session_checked");
    assert_eq!(handle.session_id(), "thread-1");
    handle
        .commit(CommitSessionOptions {
            keep_recent_count: 2,
            ..CommitSessionOptions::default()
        })
        .await
        .expect("handle commit");
    handle
        .get_session_context(2048)
        .await
        .expect("handle context");
    handle.get_archive("arc-1").await.expect("handle archive");
    handle.delete().await.expect("handle delete");

    let reqs = server.requests();
    assert_eq!(
        reqs[0].json_body()["parts"][2],
        json!({
            "type": "image_url",
            "image_url": { "url": "https://example.com/image.png", "detail": "high" }
        })
    );
    assert_eq!(
        reqs[1].raw_uri,
        "/api/v1/sessions/thread%23message/messages/batch"
    );
    assert_eq!(reqs[2].json_body()["auto_commit_policy"], Value::Null);
    assert_eq!(reqs[3].json_body(), json!({ "auto_commit_policy": null }));
    assert_eq!(
        reqs[4].json_body(),
        json!({
            "keep_recent_count": 0,
            "extraction_metadata": { "event": { "tags": [] } }
        })
    );
}

#[tokio::test]
async fn tasks_compile_rules_watches_snapshot_admin_acl_mcps_and_system() {
    let server = MockServer::start(|req| match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/api/v1/tasks/missing") => MockResponse::json(
            404,
            json!({ "status": "error", "error": { "code": "NOT_FOUND", "message": "missing" } }),
        ),
        ("GET", "/api/v1/tasks") => MockResponse::ok_result(json!([{ "task_id": "t1" }])),
        ("POST", "/api/v1/compile/create") => {
            MockResponse::ok_result(json!({ "compile_name": "research-summary" }))
        }
        ("POST", "/api/v1/compile") => MockResponse::ok_result(json!({ "task_id": "cmp_1" })),
        ("POST", "/api/v1/resources/temp_upload") => {
            MockResponse::ok_result(json!({ "temp_file_id": "upload_rules" }))
        }
        ("POST", "/api/v1/rules/compile") => {
            MockResponse::ok_result(json!({ "task_id": "rul_123" }))
        }
        ("POST", "/api/v1/rules/search") => MockResponse::ok_result(json!({ "rules": [] })),
        ("PATCH", "/api/v1/watches") => MockResponse::ok_result(json!({ "is_active": false })),
        ("DELETE", "/api/v1/watches/w-1") => MockResponse::ok_result(json!({ "deleted": true })),
        ("POST", "/api/v1/watches/w-1/trigger") => {
            MockResponse::ok_result(json!({ "triggered": true }))
        }
        ("POST", "/api/v1/snapshot/commit") => {
            MockResponse::ok_result(json!({ "commit": "abc123" }))
        }
        ("POST", "/api/v1/snapshot/restore") => {
            MockResponse::ok_result(json!({ "restored": true }))
        }
        ("GET", "/api/v1/snapshot/show") => {
            if req.query_param("path").is_some() {
                MockResponse::binary(
                    200,
                    b"blob-bytes".to_vec(),
                    vec![("X-Snapshot-Oid", "oid123"), ("X-Snapshot-Size", "10")],
                )
            } else {
                MockResponse::ok_result(json!({ "oid": "commit123" }))
            }
        }
        ("GET", "/api/v1/snapshot/log") => MockResponse::ok_result(json!([{ "oid": "commit123" }])),
        ("GET", "/api/v1/snapshot/diff") => MockResponse::ok_result(json!({ "patch": "@@" })),
        ("GET", "/api/v1/snapshot/diff-paths") => MockResponse::ok_result(
            json!({ "paths": ["viking://resources/a.md"], "has_more": false }),
        ),
        ("GET", "/api/v1/snapshot/ignore") => MockResponse::ok_result(json!("*.tmp\n")),
        ("PUT", "/api/v1/snapshot/ignore") | ("DELETE", "/api/v1/snapshot/ignore") => {
            MockResponse::ok_result(json!(null))
        }
        ("PUT", "/api/v1/admin/accounts/acme/groups/engineering/members") => {
            assert_eq!(req.json_body(), json!({ "user_ids": ["alice"] }));
            MockResponse::ok_result(json!({ "added": true }))
        }
        ("DELETE", "/api/v1/admin/accounts/acme/groups/engineering/members") => {
            assert_eq!(req.json_body(), json!({ "user_ids": ["alice"] }));
            MockResponse::ok_result(json!({ "removed": true }))
        }
        ("POST", "/api/v1/admin/accounts") => {
            MockResponse::ok_result(json!({ "account_id": "acme" }))
        }
        ("PUT", "/api/v1/acl") => MockResponse::ok_result(json!({ "acl_mode": "restricted" })),
        ("POST", "/api/v1/skills") => {
            MockResponse::ok_result(json!({ "uri": "viking://~/skills/demo" }))
        }
        ("POST", "/api/v1/mcps") => MockResponse::ok_result(json!({ "name": "srv" })),
        ("POST", "/api/v1/mcps/mount") => MockResponse::ok_result(json!({ "mounted": true })),
        ("GET", "/api/v1/observer/system") => {
            MockResponse::ok_result(json!({ "is_healthy": true }))
        }
        ("GET", "/api/v1/agent-evolution/experiences/trajectories") => {
            MockResponse::ok_result(json!({ "trajectories": [] }))
        }
        ("GET", "/api/v1/agent-evolution/experiences/outcomes") => {
            MockResponse::ok_result(json!({ "total": 0 }))
        }
        ("POST", "/api/v1/openviking-assets/resolve") => {
            MockResponse::ok_result(json!({ "valid": true }))
        }
        ("POST", "/api/v1/openviking-assets/preflight") => {
            MockResponse::ok_result(json!({ "ok": true }))
        }
        other => panic!("unexpected endpoint: {other:?}"),
    })
    .await;

    let client = AsyncHttpClient::builder()
        .url(server.base_url())
        .without_config_file()
        .build_with_env(&TestEnv::default())
        .expect("client");

    // Tasks
    assert_eq!(
        client.get_task("missing").await.expect("get_task 404"),
        None
    );
    assert_eq!(
        client
            .list_tasks(ListTasksOptions::default())
            .await
            .expect("list_tasks")
            .len(),
        1
    );

    // Compile validation & named/memory execution
    assert!(client
        .create_compile_task(
            "",
            &["viking://resources/source".to_owned()],
            "viking://resources/output",
            "viking://agent/skills/wiki",
            CompileOptions::default(),
        )
        .await
        .is_err());

    client
        .create_compile_task(
            "research-summary",
            &["viking://resources/source".to_owned()],
            "viking://resources/output",
            "viking://agent/skills/wiki",
            CompileOptions::default(),
        )
        .await
        .expect("create_compile_task");

    assert!(client
        .compile(CompileRequest::Named {
            compile_name: "research-summary".to_owned(),
            options: CompileOptions {
                instruction: Some("forbidden override".to_owned()),
                ..CompileOptions::default()
            },
        })
        .await
        .is_err());

    client
        .compile(CompileRequest::Full {
            from_uris: Vec::new(),
            to: "viking://~/memories".to_owned(),
            skill: "memory".to_owned(),
            options: CompileOptions::default(),
        })
        .await
        .expect("compile memory");

    // Rules compile (local file upload + remote URL) & search
    let dir = tempdir().expect("tempdir");
    let rules_file = dir.path().join("rules.md");
    std::fs::write(&rules_file, "Always run focused tests.").expect("write rules.md");
    client
        .rule_compile(
            Some("Compile these rules"),
            RuleCompileOptions {
                sources: vec![
                    RuleCompileSource::from(rules_file.as_path()),
                    RuleCompileSource::from("https://example.com/team-rules.md"),
                ],
            },
        )
        .await
        .expect("rule_compile");
    client
        .rule_search(
            "prepare a release",
            RuleSearchOptions {
                rule_probability_threshold: Some(0.8),
                ..RuleSearchOptions::default()
            },
        )
        .await
        .expect("rule_search");

    // Watches
    client
        .update_watch(UpdateWatchOptions {
            target: WatchRef::by_to_uri("/resources/demo"),
            is_active: Some(false),
            ..UpdateWatchOptions::default()
        })
        .await
        .expect("update_watch");
    client
        .trigger_watch(WatchRef::by_task_id("w-1"))
        .await
        .expect("trigger_watch");
    client
        .delete_watch(WatchRef::by_task_id("w-1"))
        .await
        .expect("delete_watch");

    // Snapshot namespace (`commit`, `restore`, `show` commit & binary blob, `log`, `diff`, `diff_paths`, `ignore`)
    let snap = client.snapshot();
    snap.commit(SnapshotCommitOptions::new("init"))
        .await
        .expect("snap commit");
    snap.restore(SnapshotRestoreOptions::new("abc123"))
        .await
        .expect("snap restore");
    match snap.show("HEAD", None).await.expect("show commit") {
        SnapshotShowResult::Commit(c) => assert_eq!(c.get("oid"), Some(&json!("commit123"))),
        SnapshotShowResult::Blob(_) => panic!("expected commit json"),
    }
    match snap
        .show("HEAD", Some("viking://resources/a.bin"))
        .await
        .expect("show blob")
    {
        SnapshotShowResult::Blob(b) => {
            assert_eq!(b.oid, "oid123");
            assert_eq!(b.size, 10);
            assert_eq!(b.bytes, b"blob-bytes");
        }
        SnapshotShowResult::Commit(_) => panic!("expected blob"),
    }
    assert_eq!(
        snap.log(SnapshotLogOptions::default())
            .await
            .expect("snap log")
            .len(),
        1
    );
    snap.diff("viking://resources/a.md", "HEAD", None)
        .await
        .expect("snap diff");
    snap.diff_paths(
        &["viking://resources".to_owned()],
        SnapshotDiffPathsOptions::new("HEAD"),
    )
    .await
    .expect("snap diff_paths");
    assert_eq!(snap.get_gitignore().await.expect("get ignore"), "*.tmp\n");
    snap.set_gitignore("*.tmp\n").await.expect("set ignore");
    snap.delete_gitignore().await.expect("delete ignore");

    // Admin group batch membership contract + ACL + Skills + MCP + Observer + Evolution/Assets
    client
        .admin_create_account("acme", "root", AdminUserInitOptions::default())
        .await
        .expect("create account");
    client
        .admin_add_group_member("acme", "engineering", "alice")
        .await
        .expect("add group member");
    client
        .admin_remove_group_member("acme", "engineering", "alice")
        .await
        .expect("remove group member");

    client
        .acl_set(
            "/resources/demo",
            Some(&[AclEntry {
                principal: "user:alice".to_owned(),
                level: "read".to_owned(),
            }]),
            Some("restricted"),
        )
        .await
        .expect("acl_set");

    client
        .add_skill(
            SkillSource::from("---\nname: demo\n---\n# Demo\n"),
            AddSkillOptions::default(),
        )
        .await
        .expect("add_skill inline");

    client
        .add_mcp(
            "srv",
            "http://mcp.local/mcp",
            None,
            McpScopeOptions::default(),
        )
        .await
        .expect("add_mcp");
    client
        .mount_mcp_tool(
            "viking://agent/tools/srv/ping.json",
            "viking://resources/ping.md",
            "Ping",
            MountMcpToolOptions::default(),
        )
        .await
        .expect("mount_mcp_tool");

    assert!(client
        .observer()
        .is_healthy()
        .await
        .expect("observer healthy"));
    let _ = client
        .get_status(Some(ObserverFormat::Table))
        .await
        .expect("get_status table");

    client
        .list_experience_trajectories(
            "viking://~/memories/experiences/exp1",
            ExperienceTrajectoryOptions::default(),
        )
        .await
        .expect("trajectories");
    client
        .get_experience_outcomes(
            "viking://~/memories/experiences/exp1",
            ExperienceOutcomeOptions::default(),
        )
        .await
        .expect("outcomes");
    client
        .resolve_openviking_assets("version: 1\n", ResolveAssetsOptions::default())
        .await
        .expect("resolve assets");
    client
        .preflight_openviking_asset(
            "docs",
            "https://github.com/volcengine/OpenViking.git",
            PreflightAssetOptions::default(),
        )
        .await
        .expect("preflight asset");
}
