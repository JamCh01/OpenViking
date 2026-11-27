// Copyright (c) 2026 Beijing Volcano Engine Technology Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Router;
use openviking_sdk::EnvSource;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

#[derive(Debug, Clone)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub raw_uri: String,
    pub query: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl RecordedRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }

    pub fn json_body(&self) -> Value {
        serde_json::from_slice(&self.body).expect("request body should be valid JSON")
    }

    pub fn query_map(&self) -> BTreeMap<String, Vec<String>> {
        let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
        if let Some(q) = &self.query {
            for (k, v) in url::form_urlencoded::parse(q.as_bytes()) {
                out.entry(k.into_owned()).or_default().push(v.into_owned());
            }
        }
        out
    }

    pub fn query_param(&self, key: &str) -> Option<String> {
        self.query_map()
            .get(key)
            .and_then(|vals| vals.first().cloned())
    }
}

#[derive(Debug, Clone)]
pub struct MockResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl MockResponse {
    pub fn json(status: u16, payload: Value) -> Self {
        Self {
            status,
            headers: vec![("content-type".to_owned(), "application/json".to_owned())],
            body: serde_json::to_vec(&payload).expect("serialize mock json"),
        }
    }

    pub fn ok_result(result: Value) -> Self {
        Self::json(200, json!({ "status": "ok", "result": result }))
    }

    pub fn binary(
        status: u16,
        bytes: impl Into<Vec<u8>>,
        extra_headers: Vec<(&str, &str)>,
    ) -> Self {
        let mut headers = vec![(
            "content-type".to_owned(),
            "application/octet-stream".to_owned(),
        )];
        for (k, v) in extra_headers {
            headers.push((k.to_owned(), v.to_owned()));
        }
        Self {
            status,
            headers,
            body: bytes.into(),
        }
    }

    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.push((key.to_owned(), value.to_owned()));
        self
    }
}

type HandlerFn = dyn Fn(&RecordedRequest) -> MockResponse + Send + Sync + 'static;

#[derive(Clone)]
struct ServerState {
    recorded: Arc<Mutex<Vec<RecordedRequest>>>,
    handler: Arc<HandlerFn>,
}

pub struct MockServer {
    base_url: String,
    recorded: Arc<Mutex<Vec<RecordedRequest>>>,
    task: JoinHandle<()>,
}

impl MockServer {
    pub async fn start<F>(handler: F) -> Self
    where
        F: Fn(&RecordedRequest) -> MockResponse + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local test listener");
        let addr = listener.local_addr().expect("listener local_addr");
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let state = ServerState {
            recorded: Arc::clone(&recorded),
            handler: Arc::new(handler),
        };

        let app = Router::new().fallback(handle_any).with_state(state);
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        Self {
            base_url: format!("http://{addr}"),
            recorded,
            task,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.recorded
            .lock()
            .expect("lock recorded requests")
            .clone()
    }

    pub fn clear_requests(&self) {
        self.recorded
            .lock()
            .expect("lock recorded requests")
            .clear();
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn handle_any(State(state): State<ServerState>, req: Request) -> Response {
    let method = req.method().as_str().to_owned();
    let uri = req.uri().clone();
    let path = uri.path().to_owned();
    let raw_uri = uri.to_string();
    let query = uri.query().map(str::to_owned);

    let mut headers = BTreeMap::new();
    for (name, val) in req.headers() {
        if let Ok(text) = val.to_str() {
            headers.insert(name.as_str().to_ascii_lowercase(), text.to_owned());
        }
    }

    let body_bytes: Bytes = axum::body::to_bytes(req.into_body(), 16 * 1024 * 1024)
        .await
        .expect("read request body");
    let recorded = RecordedRequest {
        method,
        path,
        raw_uri,
        query,
        headers,
        body: body_bytes.to_vec(),
    };

    state
        .recorded
        .lock()
        .expect("lock recorded requests")
        .push(recorded.clone());

    let mock_resp = (state.handler)(&recorded);
    let status = StatusCode::from_u16(mock_resp.status).expect("valid HTTP status");
    let mut header_map = HeaderMap::new();
    for (k, v) in mock_resp.headers {
        let name = HeaderName::from_bytes(k.as_bytes()).expect("valid header name");
        let value = HeaderValue::from_str(&v).expect("valid header value");
        header_map.insert(name, value);
    }
    (status, header_map, mock_resp.body).into_response()
}

#[derive(Debug, Clone, Default)]
pub struct TestEnv {
    vars: BTreeMap<String, String>,
    home: Option<PathBuf>,
}

impl TestEnv {
    pub fn with_var(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.vars.insert(key.into(), val.into());
        self
    }

    pub fn with_home(mut self, home: impl Into<PathBuf>) -> Self {
        self.home = Some(home.into());
        self
    }
}

impl EnvSource for TestEnv {
    fn get_var(&self, key: &str) -> Option<String> {
        self.vars.get(key).cloned()
    }

    fn home_dir(&self) -> Option<PathBuf> {
        self.home.clone()
    }
}
