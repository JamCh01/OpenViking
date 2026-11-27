// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

mod common;

use std::io::{Cursor, Read};
use std::path::Path;

use common::{MockResponse, MockServer, TestEnv};
use openviking_sdk::{
    AddResourceOptions, AsyncHttpClient, ErrorKind, ImportPackOptions, OPENVIKING_CLI_CONFIG_ENV,
};
use serde_json::json;
use tempfile::tempdir;
use zip::ZipArchive;

fn extract_zip_from_multipart(body: &[u8]) -> Vec<u8> {
    let start = body
        .windows(4)
        .position(|w| w == b"PK\x03\x04")
        .expect("multipart body must contain PK zip header");
    let after = &body[start..];
    // Locate the closing multipart boundary `\r\n--` after the End of Central Directory record.
    let eocd_pos = after
        .windows(4)
        .rposition(|w| w == b"PK\x05\x06")
        .expect("zip archive must contain EOCD");
    let zip_len = eocd_pos + 22;
    after[..zip_len].to_vec()
}

#[tokio::test]
async fn zip_directory_uses_forward_slashes_skips_symlinks_and_cleans_temp_archive() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("project");
    let nested = source.join("subdir");
    std::fs::create_dir_all(&nested).expect("mkdir nested");
    std::fs::write(source.join("same.txt"), b"root").expect("write root");
    std::fs::write(nested.join("same.txt"), b"nested").expect("write nested");

    let outside = dir.path().join("outside.txt");
    std::fs::write(&outside, b"outside").expect("write outside");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, source.join("file-link")).expect("symlink outside");
        std::os::unix::fs::symlink(source.join("same.txt"), source.join("inside-link"))
            .expect("symlink inside");
        std::os::unix::fs::symlink(dir.path(), source.join("dir-link")).expect("symlink dir");
    }

    let archives_dir = dir.path().join("archives");
    std::fs::create_dir_all(&archives_dir).expect("mkdir archives");

    let server = MockServer::start(|req| {
        if req.path == "/api/v1/resources/temp_upload" {
            let zip_bytes = extract_zip_from_multipart(&req.body);
            let mut archive = ZipArchive::new(Cursor::new(zip_bytes)).expect("open zip");
            let mut names = Vec::new();
            for i in 0..archive.len() {
                names.push(archive.by_index(i).expect("entry").name().to_owned());
            }
            names.sort();
            assert_eq!(
                names,
                vec!["same.txt".to_owned(), "subdir/same.txt".to_owned()]
            );

            let mut root_buf = Vec::new();
            archive
                .by_name("same.txt")
                .expect("same.txt")
                .read_to_end(&mut root_buf)
                .expect("read root");
            assert_eq!(root_buf, b"root");

            let mut nested_buf = Vec::new();
            archive
                .by_name("subdir/same.txt")
                .expect("subdir/same.txt")
                .read_to_end(&mut nested_buf)
                .expect("read nested");
            assert_eq!(nested_buf, b"nested");

            return MockResponse::ok_result(json!({ "temp_file_id": "upload-1" }));
        }

        assert_eq!(req.path, "/api/v1/resources");
        assert_eq!(
            req.json_body(),
            json!({
                "source_name": "project",
                "temp_file_id": "upload-1",
                "wait": false
            })
        );
        MockResponse::ok_result(json!({ "root_uri": "viking://resources/project" }))
    })
    .await;

    let client = AsyncHttpClient::builder()
        .url(server.base_url())
        .temp_dir(&archives_dir)
        .without_config_file()
        .build_with_env(&TestEnv::default())
        .expect("build client");

    let res = client
        .add_resource(&source.to_string_lossy(), AddResourceOptions::default())
        .await
        .expect("add_resource directory");
    assert_eq!(
        res.get("root_uri"),
        Some(&json!("viking://resources/project"))
    );

    let paths: Vec<String> = server.requests().into_iter().map(|r| r.path).collect();
    assert_eq!(
        paths,
        vec!["/api/v1/resources/temp_upload", "/api/v1/resources"]
    );
    assert_eq!(
        std::fs::read_dir(&archives_dir).expect("read_dir").count(),
        0,
        "temporary zip archive must be cleaned up"
    );
}

#[tokio::test]
async fn upload_temp_file_preflights_gateway_and_forwards_upload_mode() {
    let server = MockServer::start(|req| {
        if req.header("x-gateway-token").is_none() {
            return MockResponse::json(401, json!({ "detail": "gateway auth" }))
                .with_header("X-VikingBot-Gateway", "true");
        }
        if req.path == "/api/v1/resources/temp_upload" {
            let body_str = String::from_utf8_lossy(&req.body);
            assert!(body_str.contains("filename=\"demo.md\""));
            assert!(body_str.contains("# Demo\n"));
            assert!(body_str.contains("name=\"upload_mode\""));
            return MockResponse::ok_result(json!({ "temp_file_id": "upload-1" }));
        }
        MockResponse::ok_result(json!({ "root_uri": "viking://resources/demo" }))
    })
    .await;

    let dir = tempdir().expect("tempdir");
    let conf_path = dir.path().join("ovcli.conf");
    std::fs::write(
        &conf_path,
        serde_json::to_vec(&json!({
            "url": server.base_url(),
            "gateway_token": "gateway-token",
            "upload": {
                "mode": "shared",
                "ignore_dirs": ".cache",
                "include": "*.md",
                "exclude": "*.tmp"
            }
        }))
        .expect("json"),
    )
    .expect("write");

    let source = dir.path().join("demo.md");
    std::fs::write(&source, b"# Demo\n").expect("write demo.md");

    let env = TestEnv::default().with_var(
        OPENVIKING_CLI_CONFIG_ENV,
        conf_path.to_string_lossy().into_owned(),
    );

    let client = AsyncHttpClient::builder()
        .build_with_env(&env)
        .expect("build client");
    let res = client
        .add_resource(&source.to_string_lossy(), AddResourceOptions::default())
        .await
        .expect("upload with gateway preflight");
    assert_eq!(res.get("root_uri"), Some(&json!("viking://resources/demo")));

    let reqs = server.requests();
    let trace: Vec<(String, String, Option<String>)> = reqs
        .iter()
        .map(|r| {
            (
                r.method.clone(),
                r.path.clone(),
                r.header("x-gateway-token").map(str::to_owned),
            )
        })
        .collect();
    assert_eq!(
        trace,
        vec![
            ("GET".to_owned(), "/health".to_owned(), None),
            (
                "POST".to_owned(),
                "/api/v1/resources/temp_upload".to_owned(),
                Some("gateway-token".to_owned()),
            ),
            ("POST".to_owned(), "/api/v1/resources".to_owned(), None),
            (
                "POST".to_owned(),
                "/api/v1/resources".to_owned(),
                Some("gateway-token".to_owned()),
            ),
        ]
    );
    assert!(String::from_utf8_lossy(&reqs[1].body).contains("shared"));

    // Explicit override to "local"
    server.clear_requests();
    let local_client = AsyncHttpClient::builder()
        .upload_mode("local")
        .build_with_env(&env)
        .expect("build local upload client");
    local_client
        .add_resource(&source.to_string_lossy(), AddResourceOptions::default())
        .await
        .expect("upload local override");
    let local_reqs = server.requests();
    assert!(String::from_utf8_lossy(&local_reqs[1].body).contains("local"));
}

#[tokio::test]
async fn ovpack_export_backup_and_import_validation_match_python_sdk() {
    let server = MockServer::start(|req| match req.path.as_str() {
        "/api/v1/pack/export" => MockResponse::binary(200, b"exported".to_vec(), vec![]),
        "/api/v1/pack/backup" => MockResponse::binary(200, b"backup".to_vec(), vec![]),
        "/api/v1/resources/temp_upload" => {
            MockResponse::ok_result(json!({ "temp_file_id": "upload_pack.ovpack" }))
        }
        "/api/v1/pack/import" => {
            assert_eq!(
                req.json_body(),
                json!({
                    "parent": "viking://resources/",
                    "on_conflict": "skip",
                    "temp_file_id": "upload_pack.ovpack"
                })
            );
            MockResponse::ok_result(json!({ "uri": "viking://resources/imported" }))
        }
        other => panic!("unexpected path: {other}"),
    })
    .await;

    let client = AsyncHttpClient::builder()
        .url(server.base_url())
        .without_config_file()
        .build_with_env(&TestEnv::default())
        .expect("build client");

    let dir = tempdir().expect("tempdir");
    let exports_dir = dir.path().join("exports");
    std::fs::create_dir_all(&exports_dir).expect("mkdir exports");
    std::fs::write(exports_dir.join("demo.ovpack"), b"old-backup").expect("seed existing");

    let export_path = client
        .export_ovpack(
            "/resources/demo/",
            &exports_dir.join("demo").to_string_lossy(),
            false,
        )
        .await
        .expect("export_ovpack");
    assert!(export_path.ends_with("demo.ovpack"));
    assert_eq!(
        std::fs::read(&export_path).expect("read export"),
        b"exported"
    );

    let backup_path = client
        .backup_ovpack(&dir.path().join("backup-dir").to_string_lossy(), false)
        .await
        .expect("backup_ovpack");
    assert!(backup_path.ends_with("backup-dir.ovpack"));
    assert_eq!(std::fs::read(&backup_path).expect("read backup"), b"backup");

    // Import uploads local file
    let import_uri = client
        .import_ovpack(
            &export_path,
            "viking://resources/",
            ImportPackOptions {
                on_conflict: Some("skip".to_owned()),
                vector_mode: None,
            },
        )
        .await
        .expect("import_ovpack");
    assert_eq!(import_uri, "viking://resources/imported");

    // Fast-fail when local file is missing or is a directory (zero HTTP requests sent)
    server.clear_requests();
    let missing_err = client
        .import_ovpack(
            &dir.path().join("missing.ovpack").to_string_lossy(),
            "viking://resources/",
            ImportPackOptions::default(),
        )
        .await
        .expect_err("missing ovpack must fail fast");
    assert_eq!(missing_err.kind(), ErrorKind::Io);
    assert!(missing_err
        .to_string()
        .contains("Local ovpack file not found"));

    let dir_err = client
        .import_ovpack(
            &Path::new(dir.path()).to_string_lossy(),
            "viking://resources/",
            ImportPackOptions::default(),
        )
        .await
        .expect_err("directory path must fail fast");
    assert_eq!(dir_err.kind(), ErrorKind::InvalidArgument);
    assert!(dir_err.message().contains("is not a file"));
    assert!(server.requests().is_empty());
}
