//! 真机冒烟：启动真实 llama-server 加载真实模型，经 `TranslationService` 走完整链路。
//!
//! 默认忽略（需要本机有 llama-server 和 GGUF 模型）。运行方式（PowerShell）：
//! ```text
//! $env:TRANS_LLAMA_SERVER = 'F:\my_ai\subgen\dist\translator\llama-server.exe'
//! $env:TRANS_LOCAL_MODEL  = 'F:\hf_cache\Qwen3-4B-Instruct-2507-Q4_K_M.gguf'
//! cargo test -p trans-providers --test local_engine_smoke -- --ignored --nocapture
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use trans_application::ports::{ChatEngine, EngineKind};
use trans_application::router::{EngineSet, RouteError};
use trans_application::service::{
    CalibrateRequest, ServiceConfig, TranslateError, TranslateRequest, TranslationService,
};
use trans_application::settings::AppSettings;
use trans_domain::glossary::GlossaryEntry;
use trans_domain::lang::{Direction, Tone};
use trans_providers::llama_server::{LlamaServer, LlamaServerConfig};
use trans_providers::openai_compat::{OpenAiCompatConfig, OpenAiCompatEngine};

fn env_path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run this smoke test")))
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a real llama-server binary and GGUF model"]
async fn translates_through_a_real_local_llama_server() {
    let config = LlamaServerConfig {
        server_path: env_path("TRANS_LLAMA_SERVER"),
        model_path: env_path("TRANS_LOCAL_MODEL"),
        threads: 16,
        context_size: 4096,
        port: 18765, // 避开默认端口，不和正在运行的应用冲突
    };

    let started = Instant::now();
    let server =
        LlamaServer::start(config.clone(), Duration::from_secs(180)).await.expect("llama-server should become healthy");
    println!("llama-server ready in {} ms", started.elapsed().as_millis());

    let engine = OpenAiCompatEngine::new(OpenAiCompatConfig {
        kind: EngineKind::Local,
        base_url: format!("{}/v1", config.base_url()),
        model: "local-smoke".into(),
        api_key: None,
        reasoning_effort: None,
    })
    .unwrap();
    let engines = EngineSet { local: Some(Arc::new(engine) as Arc<dyn ChatEngine>), online: None };
    let service = TranslationService::new(engines, ServiceConfig::from(&AppSettings::default()));

    let warm = Instant::now();
    service.warm_up_local().await.expect("warm-up should succeed");
    println!(
        "warm-up took {} ms (first translation below should no longer pay for the system prompt)",
        warm.elapsed().as_millis()
    );

    let glossary = vec![
        GlossaryEntry { foreign: "WTS".into(), user: "出售".into() },
        GlossaryEntry { foreign: "pm".into(), user: "私聊".into() },
        GlossaryEntry { foreign: "Enigma".into(), user: "谜团".into() },
    ];
    let cases = [
        (Direction::Incoming, "[Trade] Stormrage: WTS Enigma 2.5b, pm me"),
        (Direction::Incoming, "Could you do $40 including shipping to Austin, TX?"),
        (Direction::Outgoing, "我用 2 个 Ber 换你的谜团可以吗？"),
    ];
    for (direction, text) in cases {
        let request = TranslateRequest { direction, text: text.into(), tone: Tone::Casual, glossary: glossary.clone() };
        let outcome = service.translate(request).await.expect("local translation should succeed");
        println!(
            "{direction:?} {:>5} ms | {text} -> {} | issues: {:?}",
            outcome.elapsed_ms, outcome.translation, outcome.issues
        );
        assert_eq!(outcome.engine, EngineKind::Local);
        assert!(!outcome.translation.trim().is_empty());
    }

    // 没配置在线引擎时，在线校对必须明确报错，不能悄悄用本地引擎顶替。
    let calibration = service
        .calibrate(CalibrateRequest {
            direction: Direction::Outgoing,
            source: "你好".into(),
            draft: "Hello".into(),
            tone: Tone::Casual,
            context: vec![],
            glossary: vec![],
        })
        .await;
    assert_eq!(calibration.unwrap_err(), TranslateError::Route(RouteError::OnlineNotConfigured));

    server.stop().await;
}
