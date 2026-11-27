// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

mod common;

use std::time::Duration;

use common::{MockResponse, MockServer, TestEnv};
use openviking_sdk::{AsyncHttpClient, ErrorKind};
use serde_json::{json, Value};

#[tokio::test]
async fn public_client_preserves_all_server_error_codes_and_details() {
    let cases: Vec<(&str, u16, ErrorKind, Value)> = vec![
        (
            "INVALID_ARGUMENT",
            400,
            ErrorKind::InvalidArgument,
            json!({ "field": "uri" }),
        ),
        (
            "INVALID_URI",
            400,
            ErrorKind::InvalidUri,
            json!({ "uri": "demo", "reason": "malformed" }),
        ),
        (
            "NOT_FOUND",
            404,
            ErrorKind::NotFound,
            json!({
                "resource": "viking://resources/demo",
                "type": "resource",
                "reason": "The data may not have been indexed yet or may have been deleted"
            }),
        ),
        (
            "ALREADY_EXISTS",
            409,
            ErrorKind::AlreadyExists,
            json!({ "resource": "demo", "type": "resource" }),
        ),
        (
            "CONFLICT",
            409,
            ErrorKind::Conflict,
            json!({ "resource": "demo" }),
        ),
        (
            "FAILED_PRECONDITION",
            412,
            ErrorKind::FailedPrecondition,
            json!({}),
        ),
        ("ABORTED", 409, ErrorKind::Aborted, json!({})),
        (
            "UNAUTHENTICATED",
            401,
            ErrorKind::Unauthenticated,
            json!({}),
        ),
        (
            "PERMISSION_DENIED",
            403,
            ErrorKind::PermissionDenied,
            json!({}),
        ),
        (
            "RESOURCE_EXHAUSTED",
            429,
            ErrorKind::ResourceExhausted,
            json!({ "upstream_status_code": 429 }),
        ),
        (
            "UNAVAILABLE",
            503,
            ErrorKind::Unavailable,
            json!({ "service": "storage backend", "reason": "lock contention" }),
        ),
        ("INTERNAL", 500, ErrorKind::Internal, json!({})),
        (
            "DEADLINE_EXCEEDED",
            504,
            ErrorKind::DeadlineExceeded,
            json!({}),
        ),
        ("UNIMPLEMENTED", 501, ErrorKind::Unimplemented, json!({})),
        ("NOT_INITIALIZED", 503, ErrorKind::NotInitialized, json!({})),
        (
            "PROCESSING_ERROR",
            500,
            ErrorKind::ProcessingError,
            json!({}),
        ),
        (
            "EMBEDDING_FAILED",
            500,
            ErrorKind::EmbeddingFailed,
            json!({}),
        ),
        ("VLM_FAILED", 500, ErrorKind::VlmFailed, json!({})),
        ("SESSION_EXPIRED", 410, ErrorKind::SessionExpired, json!({})),
        ("UNKNOWN", 500, ErrorKind::Unknown, json!({})),
        (
            "PROVIDER_SPECIFIC",
            502,
            ErrorKind::Unknown,
            json!({ "provider": { "retry_after": 2 } }),
        ),
    ];

    for (code, status, expected_kind, details) in cases {
        let code_owned = code.to_owned();
        let details_clone = details.clone();
        let server = MockServer::start(move |_| {
            MockResponse::json(
                status,
                json!({
                    "status": "error",
                    "error": {
                        "code": code_owned,
                        "message": "request failed",
                        "details": details_clone,
                    }
                }),
            )
        })
        .await;

        let client = AsyncHttpClient::builder()
            .url(server.base_url())
            .without_config_file()
            .build_with_env(&TestEnv::default())
            .expect("build client");

        let err = client
            .stat("viking://resources/demo")
            .await
            .expect_err("expected server error");

        assert_eq!(err.kind(), expected_kind, "code={code}");
        assert_eq!(err.code(), code);
        assert!(err.is_code(code));
        assert_eq!(err.status_code(), Some(status));

        if let Some(obj) = details.as_object() {
            for (k, v) in obj {
                assert_eq!(err.details().get(k), Some(v), "detail mismatch for {k}");
            }
        }
        if code == "NOT_FOUND" {
            assert!(err.is_not_found());
            assert!(err
                .to_string()
                .contains("The data may not have been indexed yet or may have been deleted"));
        }
        if code == "UNAVAILABLE" {
            assert!(err
                .to_string()
                .contains("Storage backend unavailable: lock contention"));
        }
    }
}

#[tokio::test]
async fn transport_connection_failure_maps_to_transport_error() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let closed_addr = listener.local_addr().expect("addr");
    drop(listener);

    let client = AsyncHttpClient::builder()
        .url(format!("http://{closed_addr}"))
        .timeout(Duration::from_millis(500))
        .without_config_file()
        .build_with_env(&TestEnv::default())
        .expect("build client");

    let err = client
        .stat("viking://resources/demo")
        .await
        .expect_err("connection refused must fail");
    assert_eq!(err.kind(), ErrorKind::Transport);
}
