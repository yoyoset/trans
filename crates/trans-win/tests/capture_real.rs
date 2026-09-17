//! 真机集成测试：用真实截图 + 真实 OCR 跑完整读取管道。
//!
//! 默认忽略（依赖当前桌面上开着的窗口）。运行方式（PowerShell）：
//! ```text
//! $env:TRANS_CAPTURE_WINDOW = 'chrome.exe'   # 窗口标题或进程名的一部分
//! $env:TRANS_CAPTURE_RECT   = '0,0,1200,800' # 取景框测试用的屏幕区域 x,y,宽,高
//! $env:TRANS_CAPTURE_LANG   = 'en'           # 可选，默认 en
//! cargo test -p trans-win --test capture_real -- --ignored --nocapture
//! ```

#![cfg(windows)]

use std::sync::Arc;
use std::time::Instant;

use trans_application::capture_pipeline::{CapturePipeline, CaptureSource, CaptureTarget};
use trans_domain::chat_lines::LineParser;
use trans_domain::geometry::Rect;
use trans_domain::lang::Lang;
use trans_domain::region::{AnchoredRegion, EdgeValue, Origin, Unit};
use trans_win::{GraphicsCapture, MonitorCapture, WindowsOcr};

fn pipeline() -> CapturePipeline {
    CapturePipeline::new(Arc::new(GraphicsCapture), Arc::new(MonitorCapture::default()), Arc::new(WindowsOcr))
}

fn lang() -> Lang {
    Lang::new(&std::env::var("TRANS_CAPTURE_LANG").unwrap_or_else(|_| "en".into()))
}

fn target(source: CaptureSource) -> CaptureTarget {
    CaptureTarget { source, parser: LineParser::Bubble, scale: 1.0, lang: lang() }
}

fn rect_from_env() -> Rect {
    let raw = std::env::var("TRANS_CAPTURE_RECT").unwrap_or_else(|_| "0,0,1200,800".into());
    let parts: Vec<i64> = raw.split(',').map(|p| p.trim().parse().expect("rect is x,y,width,height")).collect();
    let [x, y, width, height] = parts[..] else { panic!("TRANS_CAPTURE_RECT needs 4 numbers") };
    Rect { x: x as i32, y: y as i32, width: width as u32, height: height as u32 }
}

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
    let pipeline = pipeline();
    let windows = pipeline.list_windows().expect("window enumeration works");
    let matches = |value: &str| value.to_lowercase().contains(&needle.to_lowercase());
    let window = windows
        .iter()
        .find(|w| matches(&w.title) || matches(&w.process_name))
        .unwrap_or_else(|| panic!("no window matches {needle}; open one or pick another"));
    println!("target: {} — {}", window.process_name, window.title);

    let source = CaptureSource::Window { window: window.id, region: whole_window() };
    let started = Instant::now();
    let outcome = pipeline.poll(&target(source.clone())).expect("capture + ocr + parse works on a real window");
    println!("first poll: {} ms, region {:?}", started.elapsed().as_millis(), outcome.region);
    for message in outcome.fresh.iter().take(8) {
        println!("  {:?} | {}", message.speaker, message.text);
    }
    assert!(!outcome.all.is_empty(), "expected some text on screen; is the window empty or minimized?");

    // 同一屏再抓一次：内容没变，就不应该再报出新消息。
    let second = pipeline.poll(&target(source)).expect("second poll works");
    println!("second poll: fresh {} of {} recognized", second.fresh.len(), second.all.len());
    assert!(
        second.fresh.len() < outcome.fresh.len().max(1),
        "unchanged screen must not produce the same messages again: {:?}",
        second.fresh
    );
}

#[test]
#[ignore = "needs something readable on screen at the given rectangle"]
fn reads_messages_from_a_screen_rectangle_like_the_lens_does() {
    let rect = rect_from_env();
    let pipeline = pipeline();
    let started = Instant::now();
    let outcome = pipeline.poll(&target(CaptureSource::Screen { rect })).expect("screen region capture works");
    println!(
        "lens poll: {} ms, region {:?}, recognized {}",
        started.elapsed().as_millis(),
        outcome.region,
        outcome.all.len()
    );
    for message in outcome.all.iter().take(8) {
        println!("  {:?} | {}", message.speaker, message.text);
    }
    // 画面没变的第二轮：复用上一轮识别结果，不再跑 OCR。
    let started = Instant::now();
    let second = pipeline.poll(&target(CaptureSource::Screen { rect })).expect("second poll works");
    println!("unchanged second poll: {} ms, fresh {}", started.elapsed().as_millis(), second.fresh.len());
    assert_eq!(outcome.region, rect, "the lens must capture exactly the rectangle it was given");
    assert!(!outcome.all.is_empty(), "expected some text inside the rectangle");
}

/// 分段计时：连续几轮只截图、再对同一帧只做 OCR，看持续识别时时间花在哪；
/// 同时打印识别文字，框住一块会变化的内容（比如计时器）可以确认拿到的不是旧帧。
#[test]
#[ignore = "timing benchmark on the real desktop"]
fn times_screen_grab_and_ocr_separately() {
    use trans_application::capture::{OcrEngine, ScreenCapture};

    let rect = rect_from_env();
    let capture = MonitorCapture::default();
    let ocr = WindowsOcr;
    for round in 1..=5 {
        let started = Instant::now();
        let frame = capture.grab_region(rect).expect("grab works");
        let grab_ms = started.elapsed().as_millis();
        let started = Instant::now();
        let lines = ocr.recognize(&frame, &lang()).expect("ocr works");
        let text: Vec<&str> = lines.iter().map(|line| line.text.as_str()).collect();
        println!("round {round}: grab {grab_ms} ms, ocr {} ms: {text:?}", started.elapsed().as_millis());
        // 间隔一会儿再抓：画面在变时，常驻会话给出的必须是新内容而不是旧帧。
        std::thread::sleep(std::time::Duration::from_millis(900));
    }
}
