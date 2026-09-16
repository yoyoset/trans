//! 测试辅助：构造画面和 OCR 行，供各层测试共用（生产代码不应依赖这里）。

use crate::chat_lines::OcrLine;
use crate::frame::Frame;
use crate::geometry::Rect;

/// 纯色画面。管道测试只关心尺寸，不关心像素内容。
pub fn solid_frame(width: u32, height: u32) -> Frame {
    Frame::new(width, height, vec![0x20; width as usize * height as usize * 4]).expect("solid frame is well formed")
}

/// 按字符数估算宽度的 OCR 行，省去测试里手写外框。
#[derive(Debug, Clone)]
pub struct TestOcrLine {
    pub text: String,
    pub x: i32,
    pub y: i32,
}

impl TestOcrLine {
    pub fn new(text: &str, x: i32, y: i32) -> Self {
        Self { text: text.to_owned(), x, y }
    }

    pub fn into_line(self) -> OcrLine {
        let width = self.text.chars().count() as u32 * 8;
        OcrLine { text: self.text, bounds: Rect { x: self.x, y: self.y, width, height: 20 } }
    }
}
