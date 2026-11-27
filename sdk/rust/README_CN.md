# OpenViking Rust SDK

轻量级、纯异步的 [OpenViking](https://github.com/volcengine/OpenViking) Rust HTTP 客户端 SDK，基于 `tokio` + `reqwest`（`rustls-tls`）构建，与独立发布的 Python 版 `openviking-sdk` 实现 100% 功能对齐。

本目录采用独立 Cargo Workspace（`sdk/rust/Cargo.toml`）单独维护，不依赖 OpenViking 服务端或 C++ 原生引擎。

[English Documentation](./README.md)

## 核心特性

- **纯异步架构（`tokio` + `reqwest`）**：`AsyncHttpClient`（别名 `OpenVikingClient`）内部基于 `Arc` 共享连接池与不可变配置，支持低成本 `Clone` 与跨 Tokio 任务高并发调用。
- **完整对齐 Python SDK**：
  - 三级配置优先级：`显式 Builder 参数 > OPENVIKING_* 环境变量 > ~/.openviking/ovcli.conf`（支持未知配置项拼写纠错提示）。
  - 完整认证支持：`X-API-Key`、LDAP Basic（`auth_mode = "ldap"`）、OIDC Bearer（`auth_mode = "oidc"` 及 JWT 回退），以及 `X-VikingBot-Gateway: true` 401 网关令牌自动重试与文件上传前 `/health` 预检。
  - 任务级 Actor Peer 作用域：通过 `use_actor_peer`（基于 `tokio::task_local!`）按请求注入 `X-OpenViking-Actor-Peer`，不覆盖租户凭证头（`X-OpenViking-Account` / `X-OpenViking-User`）。
  - 全域 API 覆盖：资源导入与定时 Watch、Skill 生命周期、MCP 服务注册与 Live Context 工具挂载、声明式 `AsyncContextArranger`、虚拟文件系统（`ls_page`、`tree_page`、`read`、`read_raw`、`write`、`batch_write`、`set_tags`、`reindex`、32 位十六进制 Record ID 透传）、ACL 权限控制、语义/会话/多模态图片检索（`find`、`search`、`search_context`、`grep`、`glob`）、会话与 `SessionHandle`、异步任务/`compile`/`rule_compile`/`rule_search`、`.ovpack` 导入导出与备份恢复、Snapshot Git 版本控制（`client.snapshot()`）、多租户管理（Account / User / Group）、Agent Evolution 经验分析、OpenViking Assets 以及 Observer 可观测性接口。
- **遵循 Google Rust Style Guide**：全局启用 `#![forbid(unsafe_code)]`，库代码零 `.unwrap()` / `.expect()`，并通过 `clippy` 与 `ast-grep` 规则强制门禁检查。

## 安装

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
openviking-sdk = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

最低支持的 Rust 版本（MSRV）：**1.89**（`edition = "2021"`）。

## 快速开始

```rust
use openviking_sdk::{
    AddResourceOptions, AsyncHttpClient, CommitSessionOptions, CreateSessionOptions, FindOptions,
    Message, Result, TargetUri,
};

#[tokio::main]
async fn main() -> Result<()> {
    // 未显式指定的字段将自动从 OPENVIKING_* 环境变量或 ~/.openviking/ovcli.conf 解析
    let client = AsyncHttpClient::builder()
        .url("http://127.0.0.1:1933")
        .api_key("your-api-key")
        .account("acme")
        .user("alice")
        .build()?;

    if !client.health().await {
        eprintln!("OpenViking 服务未就绪");
    }

    // 1. 导入本地文件、目录（自动打包为 zip 上传）或远程 URL
    client
        .add_resource(
            "./docs/architecture.md",
            AddResourceOptions {
                wait: true,
                ..AddResourceOptions::default()
            },
        )
        .await?;

    // 2. 语义检索
    let hits = client
        .find(
            "认证与多租户架构",
            FindOptions {
                target_uri: Some(TargetUri::from("viking://resources")),
                limit: Some(5),
                ..FindOptions::default()
            },
        )
        .await?;
    println!("检索结果: {hits:?}");

    // 3. 会话消息与记忆抽取提交
    let created = client.create_session(CreateSessionOptions::default()).await?;
    if let Some(session_id) = created.get("session_id").and_then(|v| v.as_str()) {
        let session = client.session(session_id);
        session
            .add_message(Message::text("user", "我们的 Rust SDK MSRV 统一放宽到 1.89。"))
            .await?;
        session.commit(CommitSessionOptions::default()).await?;
    }

    Ok(())
}
```

## 任务级 Actor Peer 隔离（`use_actor_peer`）

在并发任务中复用同一个 `AsyncHttpClient`，按异步作用域动态切换 `X-OpenViking-Actor-Peer`：

```rust
use openviking_sdk::{use_actor_peer, AsyncHttpClient, FindOptions, Result};

async fn query_for_peers(client: AsyncHttpClient) -> Result<()> {
    let c1 = client.clone();
    let c2 = client.clone();

    let (r1, r2) = tokio::join!(
        use_actor_peer(Some("planner-agent"), async move {
            c1.find("发布检查清单", FindOptions::default()).await
        }),
        use_actor_peer(Some("reviewer-agent"), async move {
            c2.find("安全审查清单", FindOptions::default()).await
        }),
    );

    let _ = (r1??, r2??);
    Ok(())
}
```

## 声明式上下文编排（`AsyncContextArranger`）

```rust
use openviking_sdk::{ApplyOptions, AsyncHttpClient, LiveContextOptions, Result};

async fn sync_live_context(client: &AsyncHttpClient) -> Result<()> {
    let mut arranger = client.arranger("platform-ops", Some("viking://resources/ops"))?;
    arranger.live_context(
        "cluster-health.md",
        "viking://agent/tools/monitoring/cluster_status.json",
        "集群健康状态",
        LiveContextOptions {
            description: Some("实时集群健康快照".to_owned()),
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

## 本地开发与质量门禁

```bash
cd sdk/rust
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
ast-grep scan -c sgconfig.yml
cargo test --locked
```

## 发布流程

推送形如 `rust-sdk@<version>`（例如 `rust-sdk@0.1.0`）的 Git Tag 时，GitHub Actions 工作流 `.github/workflows/rust-sdk-release.yml` 将自动完成门禁校验并发布至 crates.io。
