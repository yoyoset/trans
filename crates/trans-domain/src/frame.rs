//! 帧缓冲与裁剪、缩放。纯像素计算，不依赖任何平台 API。

use thiserror::Error;

use crate::geometry::{Rect, Size};

/// 连续 BGRA8 像素，无行填充。
#[derive(Clone, PartialEq, Eq)]
pub struct Frame {
    width: u32,
    height: u32,
    bgra: Vec<u8>,
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame").field("width", &self.width).field("height", &self.height).finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FrameError {
    #[error("像素缓冲长度 {len} 与尺寸 {width}x{height} 不符")]
    BufferSize { len: usize, width: u32, height: u32 },
    #[error("裁剪区域超出画面 {width}x{height}")]
    OutOfBounds { width: u32, height: u32 },
}

impl Frame {
    pub fn new(width: u32, height: u32, bgra: Vec<u8>) -> Result<Self, FrameError> {
        let expected = width as usize * height as usize * 4;
        if bgra.len() != expected {
            return Err(FrameError::BufferSize { len: bgra.len(), width, height });
        }
        Ok(Self { width, height, bgra })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn size(&self) -> Size {
        Size { width: self.width, height: self.height }
    }

    pub fn bgra(&self) -> &[u8] {
        &self.bgra
    }

    pub fn crop(&self, rect: Rect) -> Result<Self, FrameError> {
        let out_of_bounds = |()| FrameError::OutOfBounds { width: self.width, height: self.height };
        if rect.x < 0 || rect.y < 0 || rect.width == 0 || rect.height == 0 {
            return Err(out_of_bounds(()));
        }
        let (x, y) = (rect.x as u32, rect.y as u32);
        if x + rect.width > self.width || y + rect.height > self.height {
            return Err(out_of_bounds(()));
        }
        let mut bgra = Vec::with_capacity(rect.width as usize * rect.height as usize * 4);
        for row in 0..rect.height {
            let start = (((y + row) * self.width + x) * 4) as usize;
            let end = start + (rect.width * 4) as usize;
            bgra.extend_from_slice(&self.bgra[start..end]);
        }
        Self::new(rect.width, rect.height, bgra)
    }

    /// 双线性放大。OCR 对小字识别率差，但放大倍数不是越大越好（M0-01 实测），所以倍数由预设决定。
    pub fn scaled(&self, factor: f32) -> Self {
        if !factor.is_finite() || (factor - 1.0).abs() < f32::EPSILON || factor <= 0.0 {
            return self.clone();
        }
        let dst_w = ((self.width as f32 * factor).round() as u32).max(1);
        let dst_h = ((self.height as f32 * factor).round() as u32).max(1);
        let mut bgra = vec![0u8; dst_w as usize * dst_h as usize * 4];
        for y in 0..dst_h {
            let src_y = ((y as f32 + 0.5) / factor - 0.5).clamp(0.0, (self.height - 1) as f32);
            let (y0, wy) = (src_y.floor() as u32, src_y - src_y.floor());
            let y1 = (y0 + 1).min(self.height - 1);
            for x in 0..dst_w {
                let src_x = ((x as f32 + 0.5) / factor - 0.5).clamp(0.0, (self.width - 1) as f32);
                let (x0, wx) = (src_x.floor() as u32, src_x - src_x.floor());
                let x1 = (x0 + 1).min(self.width - 1);
                let dst = ((y * dst_w + x) * 4) as usize;
                for channel in 0..4 {
                    let p = |px: u32, py: u32| f32::from(self.bgra[((py * self.width + px) * 4) as usize + channel]);
                    let top = p(x0, y0) * (1.0 - wx) + p(x1, y0) * wx;
                    let bottom = p(x0, y1) * (1.0 - wx) + p(x1, y1) * wx;
                    bgra[dst + channel] = (top * (1.0 - wy) + bottom * wy).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
        Self { width: dst_w, height: dst_h, bgra }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 4x2 的画面，每个像素的蓝色通道等于 x + y * 10，方便断言裁剪位置。
    fn sample() -> Frame {
        let mut bgra = Vec::new();
        for y in 0..2u32 {
            for x in 0..4u32 {
                bgra.extend_from_slice(&[(x + y * 10) as u8, 0, 0, 255]);
            }
        }
        Frame::new(4, 2, bgra).unwrap()
    }

    #[test]
    fn rejects_a_buffer_that_does_not_match_the_size() {
        assert_eq!(Frame::new(2, 2, vec![0; 8]).unwrap_err(), FrameError::BufferSize { len: 8, width: 2, height: 2 });
    }

    #[test]
    fn crop_takes_the_requested_rectangle() {
        let cropped = sample().crop(Rect { x: 1, y: 1, width: 2, height: 1 }).unwrap();
        assert_eq!((cropped.width(), cropped.height()), (2, 1));
        assert_eq!(cropped.bgra()[0], 11);
        assert_eq!(cropped.bgra()[4], 12);
    }

    #[test]
    fn crop_outside_the_frame_is_an_error_not_a_clamped_rectangle() {
        let frame = sample();
        assert!(frame.crop(Rect { x: 3, y: 0, width: 2, height: 1 }).is_err());
        assert!(frame.crop(Rect { x: -1, y: 0, width: 1, height: 1 }).is_err());
        assert!(frame.crop(Rect { x: 0, y: 0, width: 0, height: 1 }).is_err());
    }

    #[test]
    fn scaling_by_one_or_by_nonsense_returns_the_same_frame() {
        let frame = sample();
        assert_eq!(frame.scaled(1.0), frame);
        assert_eq!(frame.scaled(0.0), frame);
        assert_eq!(frame.scaled(f32::NAN), frame);
    }

    #[test]
    fn scaling_up_keeps_corner_pixels_and_grows_the_buffer() {
        let scaled = sample().scaled(2.0);
        assert_eq!((scaled.width(), scaled.height()), (8, 4));
        assert_eq!(scaled.bgra().len(), 8 * 4 * 4);
        assert_eq!(scaled.bgra()[0], 0, "top-left stays the top-left pixel");
        let last = ((3 * 8 + 7) * 4) as usize;
        assert_eq!(scaled.bgra()[last], 13, "bottom-right stays the bottom-right pixel");
    }
}
