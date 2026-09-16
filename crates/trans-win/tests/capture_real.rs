//! 真机集成测试：用真实截图 + 真实 OCR 跑完整读取管道。
//!
//! 默认忽略（依赖当前桌面上开着的窗口）。运行方式（PowerShell）：
//! ```text
//! $env:TRANS_CAPTURE_WINDOW = 'Notepad'      # 窗口标题或进程名的一部分
//! $env:TRANS_CAPTURE_LANG   = 'en'           # 可选，默认 en
//! cargo test -p trans-win --test capture_real -- --ignored --nocapture
//! ```

#![cfg(windows)]

use std::sync::Arc;
use std::time::Instant;

use trans_application::capture_pipeline::{CapturePipeline, CaptureTarget};
use trans_domain::chat_lines::LineParser;
use trans_domain::lang::Lang;
use trans_domain::region::{AnchoredRegion, EdgeValue, Origin, Unit};
use trans_win::{GraphicsCapture, WindowsOcr};

/// 整个窗口客户区。
fn whole_window() -> AnchoredRegion {
    let edge = |from| EdgeValue::new(0.0, Unit::Px, from);
    AnchoredRegion {
        left: edge(Origin::Start),
        top: edge(Origin::Start),
        right: edge(Origin::End),
        bottom: edge(Origin::End),
    }
}

#[test]
#[ignore = "needs a real window on the desktop"]
fn reads_messages_from_a_real_window() {
    let needle = std::env::var("TRANS_CAPTURE_WINDOW").expect("set TRANS_CAPTURE_WINDOW to a window title substring");
    let lang = Lang::new(&std::env::var("TRANS_CAPTURE_LANG").unwrap_or_else(|_| "en".into()));

    let pipeline = CapturePipeline::new(Arc::new(GraphicsCapture), Arc::new(WindowsOcr));
    let windows = pipeline.list_windows().expect("window enumeration works");
    let target_window = windows
        .iter()
        .find(|w| {
            w.title.to_lowercase().contains(&needle.to_lowercase())
                || w.process_name.to_lowercase().contains(&needle.to_lowercase())
        })
        .unwrap_or_else(|| panic!("no window matches {needle}; open one or pick another"));
    println!("target: {} — {}", target_window.process_name, target_window.title);

    let target = CaptureTarget {
        window: target_window.id,
        region: whole_window(),
        parser: LineParser::Bubble,
        scale: 1.0,
        lang,
    };

    let started = Instant::now();
    let outcome = pipeline.poll(&target).expect("capture + ocr + parse works on a real window");
    println!("first poll: {} ms, region {:?}", started.elapsed().as_millis(), outcome.region);
    for message in outcome.fresh.iter().take(8) {
        println!("  {:?} | {}", message.speaker, message.text);
    }
    assert!(!outcome.all.is_empty(), "expected some text on screen; is the window empty or minimized?");

    // 同一屏再抓一次：内容没变，就不应该再报出新消息。
    let second = pipeline.poll(&target).expect("second poll works");
    println!("second poll: fresh {} of {} recognized", second.fresh.len(), second.all.len());
    assert!(
        second.fresh.len() < outcome.fresh.len().max(1),
        "unchanged screen must not produce the same messages again: {:?}",
        second.fresh
    );
}
