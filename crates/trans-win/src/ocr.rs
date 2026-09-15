//! Windows.Media.Ocr 文字识别。
//!
//! 返回原始识别结果；中文字间空格等规范化由领域层 `normalize_ocr_text` 负责，不在适配器里处理。

use trans_application::capture::{Frame, OcrEngine, OcrError, OcrLine};
use trans_domain::geometry::Rect;
use trans_domain::lang::Lang;
use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::{OcrEngine as WinOcrEngine, OcrLine as WinOcrLine};
use windows::Storage::Streams::DataWriter;

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsOcr;

impl OcrEngine for WindowsOcr {
    fn recognize(&self, frame: &Frame, lang: &Lang) -> Result<Vec<OcrLine>, OcrError> {
        let language = installed_language(lang)?;
        let engine = WinOcrEngine::TryCreateFromLanguage(&language).map_err(backend)?;
        let bitmap = to_software_bitmap(frame)?;
        let result = engine.RecognizeAsync(&bitmap).map_err(backend)?.join().map_err(backend)?;

        let mut lines = Vec::new();
        for line in result.Lines().map_err(backend)? {
            lines.push(OcrLine { text: line.Text().map_err(backend)?.to_string(), bounds: line_bounds(&line)? });
        }
        Ok(lines)
    }
}

/// 系统里装的是 `en-US`、`zh-Hans-CN` 这类完整标签（M0-01 实测），按主语言子标签匹配。
fn installed_language(lang: &Lang) -> Result<Language, OcrError> {
    let primary = lang.primary().to_owned();
    let available = WinOcrEngine::AvailableRecognizerLanguages().map_err(backend)?;
    for language in available {
        let tag = language.LanguageTag().map_err(backend)?.to_string().to_ascii_lowercase();
        if tag == primary || tag.starts_with(&format!("{primary}-")) {
            return Ok(language);
        }
    }
    // 兜底再试一次直接创建，某些系统的可用列表可能不完整。
    let language = Language::CreateLanguage(&HSTRING::from(lang.as_str())).map_err(backend)?;
    if WinOcrEngine::IsLanguageSupported(&language).map_err(backend)? {
        Ok(language)
    } else {
        Err(OcrError::LanguageNotInstalled(lang.as_str().to_owned()))
    }
}

fn to_software_bitmap(frame: &Frame) -> Result<SoftwareBitmap, OcrError> {
    let expected = frame.width as usize * frame.height as usize * 4;
    if frame.bgra.len() != expected {
        return Err(OcrError::Backend(format!(
            "像素缓冲长度 {} 与尺寸 {}x{} 不符",
            frame.bgra.len(),
            frame.width,
            frame.height
        )));
    }
    let writer = DataWriter::new().map_err(backend)?;
    writer.WriteBytes(&frame.bgra).map_err(backend)?;
    let buffer = writer.DetachBuffer().map_err(backend)?;
    let width = i32::try_from(frame.width).map_err(|_| OcrError::Backend("图像过宽".into()))?;
    let height = i32::try_from(frame.height).map_err(|_| OcrError::Backend("图像过高".into()))?;
    SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, width, height).map_err(backend)
}

/// OcrLine 没有外框，取所有单词外框的并集。
fn line_bounds(line: &WinOcrLine) -> Result<Rect, OcrError> {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for word in line.Words().map_err(backend)? {
        let r = word.BoundingRect().map_err(backend)?;
        x0 = x0.min(r.X);
        y0 = y0.min(r.Y);
        x1 = x1.max(r.X + r.Width);
        y1 = y1.max(r.Y + r.Height);
    }
    if x0 > x1 {
        return Ok(Rect { x: 0, y: 0, width: 0, height: 0 });
    }
    Ok(Rect {
        x: x0.round() as i32,
        y: y0.round() as i32,
        width: (x1 - x0).round() as u32,
        height: (y1 - y0).round() as u32,
    })
}

fn backend(error: windows::core::Error) -> OcrError {
    OcrError::Backend(error.to_string())
}
