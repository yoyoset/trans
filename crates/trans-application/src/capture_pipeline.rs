//! 读取管道：取一块画面 → 放大 → OCR → 拼成消息 → 跨帧去重。
//!
//! 只做"这一帧看到了什么"，不负责翻译，也不负责定时。定时由壳层的后台任务决定。

use std::sync::{Arc, Mutex, PoisonError};

use thiserror::Error;
use trans_domain::chat_lines::{parse_lines, LineParser, ParsedMessage};
use trans_domain::frame::{Frame, FrameError};
use trans_domain::geometry::Rect;
use trans_domain::lang::Lang;
use trans_domain::reconcile::Reconciler;
use trans_domain::region::{AnchoredRegion, RegionError};

use crate::capture::{CaptureError, OcrEngine, OcrError, ScreenCapture, WindowCapture, WindowId, WindowInfo};

/// 判重时回看多少条消息。聊天滚动很快，几十条足够，也不会无限增长。
const RECONCILER_CAPACITY: usize = 60;

/// 画面从哪来。
#[derive(Debug, Clone, PartialEq)]
pub enum CaptureSource {
    /// 取景框：屏幕上的一块绝对区域（物理像素）。用户把框拖到哪里就识别哪里。
    Screen { rect: Rect },
    /// 跟随窗口：区域按锚点随窗口尺寸换算，窗口移动、改大小都不用重新框选。
    Window { window: WindowId, region: AnchoredRegion },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaptureTarget {
    pub source: CaptureSource,
    pub parser: LineParser,
    /// OCR 前的放大倍数，来自区域预设。
    pub scale: f32,
    /// 识别用的语言，通常是对方的语言。
    pub lang: Lang,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollOutcome {
    /// 这一帧新出现的消息，需要翻译。
    pub fresh: Vec<ParsedMessage>,
    /// 这一帧识别到的全部消息，用来让用户确认框选有没有对准。
    pub all: Vec<ParsedMessage>,
    /// 实际识别的区域。取景框模式下就是框本身；跟随窗口模式下随窗口尺寸变化。
    pub region: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PipelineError {
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error(transparent)]
    Ocr(#[from] OcrError),
    #[error(transparent)]
    Region(#[from] RegionError),
    #[error(transparent)]
    Frame(#[from] FrameError),
}

pub struct CapturePipeline {
    windows: Arc<dyn WindowCapture>,
    screen: Arc<dyn ScreenCapture>,
    ocr: Arc<dyn OcrEngine>,
    reconciler: Mutex<Reconciler>,
}

impl CapturePipeline {
    pub fn new(windows: Arc<dyn WindowCapture>, screen: Arc<dyn ScreenCapture>, ocr: Arc<dyn OcrEngine>) -> Self {
        Self { windows, screen, ocr, reconciler: Mutex::new(Reconciler::new(RECONCILER_CAPACITY)) }
    }

    /// 可截取的窗口列表，供"跟随窗口"模式选目标。
    pub fn list_windows(&self) -> Result<Vec<WindowInfo>, CaptureError> {
        self.windows.list_windows()
    }

    /// 换目标、换预设、重新框选后必须调用，否则新会话的内容会被当成重复而丢掉。
    pub fn reset(&self) {
        self.reconciler.lock().unwrap_or_else(PoisonError::into_inner).reset();
    }

    /// 识别停止时调用，释放截图会话等常驻资源；判重历史不受影响。
    pub fn release(&self) {
        self.screen.release();
    }

    /// 抓一帧并返回新出现的消息，同时把它们记入判重历史。
    pub fn poll(&self, target: &CaptureTarget) -> Result<PollOutcome, PipelineError> {
        let (all, region) = self.observe(target)?;
        let fresh = self.reconciler.lock().unwrap_or_else(PoisonError::into_inner).ingest(all.clone());
        Ok(PollOutcome { fresh, all, region })
    }

    /// 只看一眼当前识别结果，用于确认框选对没对准。不写入判重历史，所以预览多少次都不会
    /// 让正式识别漏掉消息。
    pub fn preview(&self, target: &CaptureTarget) -> Result<PollOutcome, PipelineError> {
        let (all, region) = self.observe(target)?;
        Ok(PollOutcome { fresh: Vec::new(), all, region })
    }

    fn observe(&self, target: &CaptureTarget) -> Result<(Vec<ParsedMessage>, Rect), PipelineError> {
        let (frame, region) = self.grab(&target.source)?;
        let scaled = frame.scaled(target.scale);
        let lines = self.ocr.recognize(&scaled, &target.lang)?;
        Ok((parse_lines(&lines, scaled.width(), &target.parser), region))
    }

    fn grab(&self, source: &CaptureSource) -> Result<(Frame, Rect), PipelineError> {
        match source {
            CaptureSource::Screen { rect } => Ok((self.screen.grab_region(*rect)?, *rect)),
            CaptureSource::Window { window, region } => {
                let frame = self.windows.grab_frame(*window)?;
                let rect = region.resolve(frame.size())?;
                Ok((frame.crop(rect)?, rect))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use trans_domain::conversation::Speaker;
    use trans_domain::geometry::Size;
    use trans_domain::region::{EdgeValue, Origin, Unit};
    use trans_domain::test_support::{solid_frame, TestOcrLine};

    use super::*;
    use crate::capture::OcrLine;

    struct FakeCapture {
        size: Size,
        error: Option<CaptureError>,
    }

    impl WindowCapture for FakeCapture {
        fn list_windows(&self) -> Result<Vec<WindowInfo>, CaptureError> {
            Ok(vec![])
        }
        fn grab_frame(&self, _: WindowId) -> Result<Frame, CaptureError> {
            match &self.error {
                Some(error) => Err(error.clone()),
                None => Ok(solid_frame(self.size.width, self.size.height)),
            }
        }
    }

    /// 记录被要求截取的屏幕区域，并按该尺寸返回画面。
    #[derive(Default)]
    struct FakeScreen {
        asked: StdMutex<Vec<Rect>>,
    }

    impl ScreenCapture for FakeScreen {
        fn grab_region(&self, rect: Rect) -> Result<Frame, CaptureError> {
            self.asked.lock().unwrap().push(rect);
            Ok(solid_frame(rect.width, rect.height))
        }
    }

    /// 按调用次数依次返回预设的识别结果，并记录收到的画面尺寸。
    struct FakeOcr {
        frames: StdMutex<Vec<(u32, u32)>>,
        scripted: StdMutex<Vec<Vec<TestOcrLine>>>,
    }

    impl FakeOcr {
        fn new(scripted: Vec<Vec<TestOcrLine>>) -> Arc<Self> {
            let scripted = scripted.into_iter().rev().collect();
            Arc::new(Self { frames: StdMutex::new(Vec::new()), scripted: StdMutex::new(scripted) })
        }
    }

    impl OcrEngine for FakeOcr {
        fn recognize(&self, frame: &Frame, _: &Lang) -> Result<Vec<OcrLine>, OcrError> {
            self.frames.lock().unwrap().push((frame.width(), frame.height()));
            let lines = self.scripted.lock().unwrap().pop().unwrap_or_default();
            Ok(lines.into_iter().map(TestOcrLine::into_line).collect())
        }
    }

    /// 跟随窗口：整宽，底部留 100px 输入框。
    fn window_source() -> CaptureSource {
        CaptureSource::Window {
            window: WindowId(1),
            region: AnchoredRegion {
                left: EdgeValue::new(0.0, Unit::Px, Origin::Start),
                top: EdgeValue::new(0.0, Unit::Px, Origin::Start),
                right: EdgeValue::new(0.0, Unit::Px, Origin::End),
                bottom: EdgeValue::new(100.0, Unit::Px, Origin::End),
            },
        }
    }

    fn target(source: CaptureSource, scale: f32) -> CaptureTarget {
        CaptureTarget { source, parser: LineParser::Bubble, scale, lang: Lang::new("en") }
    }

    fn pipeline(capture: FakeCapture, screen: Arc<FakeScreen>, ocr: Arc<FakeOcr>) -> CapturePipeline {
        CapturePipeline::new(Arc::new(capture), screen, ocr)
    }

    fn window_pipeline(size: Size, ocr: Arc<FakeOcr>) -> CapturePipeline {
        pipeline(FakeCapture { size, error: None }, Arc::new(FakeScreen::default()), ocr)
    }

    #[test]
    fn the_lens_captures_exactly_the_rectangle_it_was_given() {
        let screen = Arc::new(FakeScreen::default());
        let ocr = FakeOcr::new(vec![vec![TestOcrLine::new("Hi there!", 10, 10)]]);
        let capture = FakeCapture { size: Size { width: 1, height: 1 }, error: None };
        let pipeline = pipeline(capture, screen.clone(), ocr.clone());
        let rect = Rect { x: 1200, y: 640, width: 520, height: 300 };

        let outcome = pipeline.poll(&target(CaptureSource::Screen { rect }, 2.0)).unwrap();

        assert_eq!(screen.asked.lock().unwrap().as_slice(), [rect], "grabs the lens rectangle in screen coordinates");
        assert_eq!(ocr.frames.lock().unwrap()[0], (1040, 600), "OCR sees the scaled lens content");
        assert_eq!(outcome.region, rect);
        assert_eq!(outcome.fresh.len(), 1);
    }

    #[test]
    fn following_a_window_crops_to_the_resolved_region_and_scales_before_ocr() {
        let ocr = FakeOcr::new(vec![vec![]]);
        let pipeline = window_pipeline(Size { width: 400, height: 300 }, ocr.clone());
        let outcome = pipeline.poll(&target(window_source(), 2.0)).unwrap();
        assert_eq!(outcome.region, Rect { x: 0, y: 0, width: 400, height: 200 }, "bottom 100px are the input box");
        assert_eq!(ocr.frames.lock().unwrap()[0], (800, 400), "OCR sees the scaled crop");
    }

    #[test]
    fn only_new_messages_come_back_on_the_second_poll() {
        let first =
            vec![TestOcrLine::new("Hi there!", 10, 10), TestOcrLine::new("Is the item still available?", 10, 80)];
        let second = vec![
            TestOcrLine::new("Is the item still available?", 10, 10),
            TestOcrLine::new("Could you do 40 dollars?", 10, 80),
        ];
        let pipeline = window_pipeline(Size { width: 400, height: 300 }, FakeOcr::new(vec![first, second]));
        let target = target(window_source(), 1.0);

        let first = pipeline.poll(&target).unwrap();
        assert_eq!(first.fresh.len(), 2);
        assert_eq!(first.fresh[0].speaker, Speaker::Them);

        let second = pipeline.poll(&target).unwrap();
        assert_eq!(second.all.len(), 2, "both lines are still on screen");
        assert_eq!(second.fresh.len(), 1, "only the newly arrived line needs translating");
        assert_eq!(second.fresh[0].text, "Could you do 40 dollars?");
    }

    #[test]
    fn preview_does_not_consume_messages_that_the_real_poll_still_needs() {
        let frame = || vec![TestOcrLine::new("Hi there!", 10, 10)];
        let pipeline = window_pipeline(Size { width: 400, height: 300 }, FakeOcr::new(vec![frame(), frame()]));
        let preview = pipeline.preview(&target(window_source(), 1.0)).unwrap();
        assert!(preview.fresh.is_empty(), "preview reports nothing as new");
        assert_eq!(preview.all.len(), 1, "but shows what it recognized");
        assert_eq!(pipeline.poll(&target(window_source(), 1.0)).unwrap().fresh.len(), 1, "the real poll still sees it");
    }

    #[test]
    fn reset_makes_the_same_screen_count_as_new_again() {
        let frame = || vec![TestOcrLine::new("Hi there!", 10, 10)];
        let pipeline = window_pipeline(Size { width: 400, height: 300 }, FakeOcr::new(vec![frame(), frame()]));
        assert_eq!(pipeline.poll(&target(window_source(), 1.0)).unwrap().fresh.len(), 1);
        pipeline.reset();
        assert_eq!(pipeline.poll(&target(window_source(), 1.0)).unwrap().fresh.len(), 1);
    }

    #[test]
    fn a_minimized_window_surfaces_the_capture_error_unchanged() {
        let error = CaptureError::NoFrame { timeout_ms: 3000 };
        let pipeline = pipeline(
            FakeCapture { size: Size { width: 400, height: 300 }, error: Some(error.clone()) },
            Arc::new(FakeScreen::default()),
            FakeOcr::new(vec![]),
        );
        assert_eq!(pipeline.poll(&target(window_source(), 1.0)).unwrap_err(), PipelineError::Capture(error));
    }

    #[test]
    fn a_window_too_small_for_the_region_is_reported_as_a_region_error() {
        let pipeline = window_pipeline(Size { width: 400, height: 80 }, FakeOcr::new(vec![]));
        assert_eq!(
            pipeline.poll(&target(window_source(), 1.0)).unwrap_err(),
            PipelineError::Region(RegionError::Empty { width: 400, height: 80 })
        );
    }
}
