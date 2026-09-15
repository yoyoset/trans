//! 框选区域的锚点表示。
//!
//! 同一个预设要在不同窗口尺寸下使用：浏览器变宽时 Messenger 布局重排，游戏换分辨率时聊天框按高度缩放。
//! 因此每条边独立记录"从哪一侧量、用什么单位量"，窗口尺寸变化后重新解析成像素矩形。

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::geometry::{Rect, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Px,
    /// 相对客户区宽度的比例。
    WidthRatio,
    /// 相对客户区高度的比例（游戏 UI 通常按高度缩放）。
    HeightRatio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// 从左边 / 上边量。
    Start,
    /// 从右边 / 下边量。
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EdgeValue {
    pub value: f64,
    pub unit: Unit,
    pub from: Origin,
}

impl EdgeValue {
    pub const fn new(value: f64, unit: Unit, from: Origin) -> Self {
        Self { value, unit, from }
    }

    fn resolve(&self, axis_len: u32, client: Size) -> f64 {
        let distance = match self.unit {
            Unit::Px => self.value,
            Unit::WidthRatio => self.value * f64::from(client.width),
            Unit::HeightRatio => self.value * f64::from(client.height),
        };
        match self.from {
            Origin::Start => distance,
            Origin::End => f64::from(axis_len) - distance,
        }
    }

    /// 反向换算：用户拖动后得到的像素坐标，按本边原有的单位和起点重新表示。
    fn rebase(&self, position: f64, axis_len: u32, client: Size) -> Self {
        let distance = match self.from {
            Origin::Start => position,
            Origin::End => f64::from(axis_len) - position,
        };
        let value = match self.unit {
            Unit::Px => distance,
            Unit::WidthRatio => distance / f64::from(client.width.max(1)),
            Unit::HeightRatio => distance / f64::from(client.height.max(1)),
        };
        Self { value, ..*self }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AnchoredRegion {
    pub left: EdgeValue,
    pub top: EdgeValue,
    pub right: EdgeValue,
    pub bottom: EdgeValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegionError {
    #[error("框选区域在当前窗口尺寸 {width}x{height} 下为空，请重新框选")]
    Empty { width: u32, height: u32 },
}

impl AnchoredRegion {
    /// 解析为客户区内的像素矩形，超出客户区的部分被裁掉。
    pub fn resolve(&self, client: Size) -> Result<Rect, RegionError> {
        let (w, h) = (f64::from(client.width), f64::from(client.height));
        let x0 = self.left.resolve(client.width, client).clamp(0.0, w);
        let x1 = self.right.resolve(client.width, client).clamp(0.0, w);
        let y0 = self.top.resolve(client.height, client).clamp(0.0, h);
        let y1 = self.bottom.resolve(client.height, client).clamp(0.0, h);
        if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
            return Err(RegionError::Empty { width: client.width, height: client.height });
        }
        Ok(Rect {
            x: x0.round() as i32,
            y: y0.round() as i32,
            width: (x1 - x0).round() as u32,
            height: (y1 - y0).round() as u32,
        })
    }

    /// 用户拖动框后调用：保持每条边的单位和起点不变，只更新数值。
    pub fn rebased(&self, rect: Rect, client: Size) -> Self {
        let x1 = f64::from(rect.x) + f64::from(rect.width);
        let y1 = f64::from(rect.y) + f64::from(rect.height);
        Self {
            left: self.left.rebase(f64::from(rect.x), client.width, client),
            top: self.top.rebase(f64::from(rect.y), client.height, client),
            right: self.right.rebase(x1, client.width, client),
            bottom: self.bottom.rebase(y1, client.height, client),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AnchoredPoint {
    pub x: EdgeValue,
    pub y: EdgeValue,
}

impl AnchoredPoint {
    pub fn resolve(&self, client: Size) -> (i32, i32) {
        let x = self.x.resolve(client.width, client).clamp(0.0, f64::from(client.width));
        let y = self.y.resolve(client.height, client).clamp(0.0, f64::from(client.height));
        (x.round() as i32, y.round() as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn size(width: u32, height: u32) -> Size {
        Size { width, height }
    }

    /// 网页聊天：左右边按宽度比例，上边距顶部像素，下边距底部像素（输入框永远贴底）。
    fn web_chat() -> AnchoredRegion {
        AnchoredRegion {
            left: EdgeValue::new(0.3, Unit::WidthRatio, Origin::Start),
            right: EdgeValue::new(0.05, Unit::WidthRatio, Origin::End),
            top: EdgeValue::new(64.0, Unit::Px, Origin::Start),
            bottom: EdgeValue::new(80.0, Unit::Px, Origin::End),
        }
    }

    /// 游戏聊天框：锚定左下角，全部按高度比例。
    fn game_chat() -> AnchoredRegion {
        AnchoredRegion {
            left: EdgeValue::new(0.02, Unit::HeightRatio, Origin::Start),
            right: EdgeValue::new(0.62, Unit::HeightRatio, Origin::Start),
            top: EdgeValue::new(0.36, Unit::HeightRatio, Origin::End),
            bottom: EdgeValue::new(0.12, Unit::HeightRatio, Origin::End),
        }
    }

    #[test]
    fn web_region_follows_width_and_keeps_bottom_gap() {
        assert_eq!(web_chat().resolve(size(1000, 800)).unwrap(), Rect { x: 300, y: 64, width: 650, height: 656 });
        // 浏览器变宽、变矮：左右按比例移动，下边仍然距底部 80px。
        assert_eq!(web_chat().resolve(size(2000, 600)).unwrap(), Rect { x: 600, y: 64, width: 1300, height: 456 });
    }

    #[test]
    fn game_region_scales_with_height_from_the_bottom_left_corner() {
        assert_eq!(game_chat().resolve(size(1920, 1080)).unwrap(), Rect { x: 22, y: 691, width: 648, height: 259 });
        // 2560x1440：整体按高度等比放大，仍贴左下角。
        assert_eq!(game_chat().resolve(size(2560, 1440)).unwrap(), Rect { x: 29, y: 922, width: 864, height: 346 });
    }

    #[test]
    fn rebasing_a_dragged_rect_round_trips_and_keeps_units() {
        let client = size(1920, 1080);
        let dragged = Rect { x: 40, y: 700, width: 600, height: 240 };
        let rebased = game_chat().rebased(dragged, client);
        assert_eq!(rebased.resolve(client).unwrap(), dragged);
        assert_eq!(rebased.left.unit, Unit::HeightRatio);
        assert_eq!(rebased.bottom.from, Origin::End);
    }

    #[test]
    fn a_window_too_small_for_the_region_is_an_error_not_a_zero_rect() {
        assert_eq!(web_chat().resolve(size(1000, 100)), Err(RegionError::Empty { width: 1000, height: 100 }));
    }

    #[test]
    fn anchored_point_resolves_relative_to_client_size() {
        let input = AnchoredPoint {
            x: EdgeValue::new(0.6, Unit::WidthRatio, Origin::Start),
            y: EdgeValue::new(40.0, Unit::Px, Origin::End),
        };
        assert_eq!(input.resolve(size(1000, 800)), (600, 760));
    }
}
