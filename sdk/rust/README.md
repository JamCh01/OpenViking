# OpenViking Rust SDK

Pure-async, lightweight Rust HTTP client SDK for [OpenViking](https://github.com/volcengine/OpenViking), built with `tokio` + `reqwest` (`rustls-tls`) and 100% functionally aligned with the standalone Python `openviking-sdk`.

It ships with its own independent Cargo workspace (`sdk/rust/Cargo.toml`) and zero native C++ / server-engine dependencies.

[中文文档](./README_CN.md)

## Features

- **Pure Async (`tokio` + `reqwest`)**: Thread-safe, `Arc`-backed `AsyncHttpClient` (`OpenVikingClient`) safe to clone across concurrent tasks.
- **Full Python SDK Parity**:
  - 3-tier configuration resolution: `explicit builder args > OPENVIKING_* env vars > ~/.openviking/ovcli.conf` (with unknown-key typo hints).
  - Authentication modes: `X-API-Key`, LDAP Basic (`auth_mode = "ldap"`), OIDC Bearer (`auth_mode = "oidc"` + JWT fallback), and automatic `X-VikingBot-Gateway` 401 challenge retry with `/health` preflight before multipart uploads.
  - Task-local actor peer scoping via `use_actor_peer` (`X-OpenViking-Actor-Peer`) without overriding tenant credentials (`X-OpenViking-Account`, `X-OpenViking-User`).
  - Complete API coverage: Resources & Watches, Skills, MCP & Live Context mounts, Declarative `AsyncContextArranger`, Virtual FS (`ls_page`, `tree_page`, `read`, `read_raw`, `write`, `batch_write`, `set_tags`, `reindex`, record-ID passthrough), ACL, Semantic / Session / Multimodal Image Search (`find`, `search`, `search_context`, `grep`, `glob`), Sessions & `SessionHandle`, Tasks / `compile` / `rule_compile` / `rule_search`, `.ovpack` Export/Import/Backup/Restore, Snapshot Git (`client.snapshot()`), Multi-tenant Admin (Accounts, Users, Groups), Agent Evolution Experiences, OpenViking Assets, and Observer health.
- **Google Rust Style & Structural Lint**: `#![forbid(unsafe_code)]`, zero `.unwrap()` / `.expect()` in library code, enforced via `clippy` and `ast-grep`.

## Installation

Add `openviking-sdk` to your `Cargo.toml`:

```toml
[dependencies]
openviking-sdk = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Minimum Supported Rust Version (MSRV): **1.89** (`edition = "2021"`).

## Quick Start

```rust
use openviking_sdk::{
    AddResourceOptions, AsyncHttpClient, CommitSessionOptions, CreateSessionOptions, FindOptions,
    Message, Result, TargetUri,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Resolves missing fields from OPENVIKING_* env vars or ~/.openviking/ovcli.conf
    let client = AsyncHttpClient::builder()
        .url("http://127.0.0.1:1933")
        .api_key("your-api-key")
        .account("acme")
        .user("alice")
        .build()?;

    if !client.health().await {
        eprintln!("OpenViking server is not healthy");
    }

    // 1. Ingest a local file, directory (auto-zipped), or URL
    client
        .add_resource(
            "./docs/architecture.md",
            AddResourceOptions {
                wait: true,
                ..AddResourceOptions::default()
            },
        )
        .await?;

    // 2. Semantic retrieval
    let hits = client
        .find(
            "authentication architecture",
            FindOptions {
                target_uri: Some(TargetUri::from("viking://resources")),
                limit: Some(5),
                ..FindOptions::default()
            },
        )
        .await?;
    println!("Found: {hits:?}");

    // 3. Session lifecycle & memory extraction
    let created = client.create_session(CreateSessionOptions::default()).await?;
    if let Some(session_id) = created.get("session_id").and_then(|v| v.as_str()) {
        let session = client.session(session_id);
        session
            .add_message(Message::text("user", "We standardized on Rust 1.89 for SDKs."))
            .await?;
        session.commit(CommitSessionOptions::default()).await?;
    }

    Ok(())
}
```

## Configuration & Authentication

`AsyncHttpClient::builder().build()` (or `AsyncHttpClient::from_env()`) resolves settings in priority order:

1. Explicit `ClientBuilder` methods (`.url(...)`, `.api_key(...)`, `.account(...)`, `.user(...)`, `.actor_peer_id(...)`, `.timeout(...)`, `.auth_mode(...)`, etc.)
2. Environment variables:
   - `OPENVIKING_URL`
   - `OPENVIKING_API_KEY`
   - `OPENVIKING_ACCOUNT`
   - `OPENVIKING_USER`
   - `OPENVIKING_ACTOR_PEER_ID`
   - `OPENVIKING_TIMEOUT`
   - `OPENVIKING_AUTH_MODE` (`api_key` | `ldap` | `oidc`)
   - `OPENVIKING_USERNAME` / `OPENVIKING_PASSWORD` (for `ldap`)
   - `OPENVIKING_OIDC_TOKEN` (for `oidc`)
   - `OPENVIKING_CLI_CONFIG_FILE` (custom path to `ovcli.conf`)
3. `~/.openviking/ovcli.conf`

## Request-Scoped Actor Peer (`use_actor_peer`)

Share a single `AsyncHttpClient` across concurrent Tokio tasks while attributing each request to a distinct assistant/actor peer:

```rust
use openviking_sdk::{use_actor_peer, AsyncHttpClient, FindOptions, Result};

async fn query_for_peers(client: AsyncHttpClient) -> Result<()> {
    let c1 = client.clone();
    let c2 = client.clone();

    let (r1, r2) = tokio::join!(
        use_actor_peer(Some("planner-agent"), async move {
            c1.find("deployment checklist", FindOptions::default()).await
        }),
        use_actor_peer(Some("reviewer-agent"), async move {
            c2.find("security checklist", FindOptions::default()).await
        }),
    );

    let _ = (r1??, r2??);
    Ok(())
}
```

## Declarative Context-as-Code (`AsyncContextArranger`)

```rust
use openviking_sdk::{ApplyOptions, AsyncHttpClient, LiveContextOptions, Result};

async fn sync_live_context(client: &AsyncHttpClient) -> Result<()> {
    let mut arranger = client.arranger("platform-ops", Some("viking://resources/ops"))?;
    arranger.live_context(
        "cluster-health.md",
        "viking://agent/tools/monitoring/cluster_status.json",
        "Cluster Health",
        LiveContextOptions {
            description: Some("Live cluster health snapshot".to_owned()),
            timeout_seconds: 30,
            tags: vec!["env=prod".to_owned()],
            ..LiveContextOptions::default()
        },
    )?;

    let plan = arranger.plan().await?;
    println!("{}", plan.format_table());

    let result = arranger
        .apply(ApplyOptions {
            prune: true,
            dry_run: false,
            wait: true,
        })
        .await?;
    println!("{}", result.format_table());
    Ok(())
}
```

## Development & Verification

```bash
cd sdk/rust
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
ast-grep scan -c sgconfig.yml
cargo test --locked
```

## Release

Publishing to crates.io is automated via `.github/workflows/rust-sdk-release.yml` when pushing a tag matching `rust-sdk@<version>` (for example `rust-sdk@0.1.0`).
