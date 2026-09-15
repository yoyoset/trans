//! Windows.Media.Ocr 封装。

use std::time::Instant;

use anyhow::{anyhow, Result};
use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;

use crate::image_ops::Bgra;

pub struct OcrLine {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub struct OcrOutput {
    pub lines: Vec<OcrLine>,
    pub elapsed_ms: u128,
}

pub fn recognize(frame: &Bgra, lang: &str) -> Result<OcrOutput> {
    let language = Language::CreateLanguage(&HSTRING::from(lang))?;
    if !OcrEngine::IsLanguageSupported(&language)? {
        return Err(anyhow!("系统未安装 OCR 语言 {lang}"));
    }
    let engine = OcrEngine::TryCreateFromLanguage(&language)?;
    let bitmap = to_software_bitmap(frame)?;

    let started = Instant::now();
    let result = engine.RecognizeAsync(&bitmap)?.join()?;
    let elapsed_ms = started.elapsed().as_millis();

    let mut lines = Vec::new();
    for line in result.Lines()? {
        let (x, y, w, h) = line_bounds(&line)?;
        lines.push(OcrLine { text: line.Text()?.to_string(), x, y, w, h });
    }
    Ok(OcrOutput { lines, elapsed_ms })
}

fn to_software_bitmap(frame: &Bgra) -> Result<SoftwareBitmap> {
    let writer = DataWriter::new()?;
    writer.WriteBytes(&frame.pixels)?;
    let buffer = writer.DetachBuffer()?;
    Ok(SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, frame.width as i32, frame.height as i32)?)
}

/// OcrLine 本身没有外框，取所有单词外框的并集。
fn line_bounds(line: &windows::Media::Ocr::OcrLine) -> Result<(f32, f32, f32, f32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for word in line.Words()? {
        let r = word.BoundingRect()?;
        x0 = x0.min(r.X);
        y0 = y0.min(r.Y);
        x1 = x1.max(r.X + r.Width);
        y1 = y1.max(r.Y + r.Height);
    }
    if x0 == f32::MAX {
        return Ok((0.0, 0.0, 0.0, 0.0));
    }
    Ok((x0, y0, x1 - x0, y1 - y0))
}
