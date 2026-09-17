//! 真机：启动真实的 `opencode serve`，走完 启动 → 带密码访问 → 建会话 → 发消息 → 删会话 的整条链路。
//!
//! 故意使用不存在的模型：opencode 直接返回 HTTP 500，不会调用任何真实模型、不产生费用。
//! 默认忽略（需要本机装有 opencode）。运行方式：
//! ```text
//! cargo test -p trans-providers --test opencode_real -- --ignored --nocapture
//! ```

use std::time::{Duration, Instant};

use trans_application::ports::{ChatEngine, CompletionOptions, ProviderError};
use trans_domain::prompt::{ChatMessage, Role};
use trans_providers::opencode::{find_opencode, OpencodeServer, OpencodeServerConfig};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs opencode installed"]
async fn talks_to_a_real_opencode_server_without_calling_a_model() {
    let appdata = std::env::var_os("APPDATA").map(std::path::PathBuf::from);
    let program = find_opencode(std::env::var("PATH").ok().as_deref(), appdata.as_deref())
        .expect("opencode.exe should be on PATH or in the npm global folder");
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("opencode.log");
    let config = OpencodeServerConfig { program, work_dir: dir.path().join("work") };

    let started = Instant::now();
    let server = OpencodeServer::start(config, Some(&log), Duration::from_secs(60)).await.expect("serve starts");
    println!("opencode serve ready in {} ms", started.elapsed().as_millis());

    let engine = server.engine("nonexistent-provider/no-such-model", None).await.expect("tool list is reachable");
    let messages = vec![
        ChatMessage { role: Role::System, content: "test".into() },
        ChatMessage { role: Role::User, content: "hello".into() },
    ];
    let options = CompletionOptions { temperature: 0.0, max_tokens: 16, timeout: Duration::from_secs(30) };
    let error = engine.complete(&messages, &options).await.expect_err("an unknown model must fail");
    println!("unknown model -> {error:?}");
    assert!(matches!(error, ProviderError::Http { status: 500, .. }), "got {error:?}");

    server.stop().await;
}
