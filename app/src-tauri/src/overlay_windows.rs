//! 三个窗口之间的显示关系：主窗口、取景框、悬浮按钮。
//!
//! - 关主窗口不退出，而是收成桌面上的悬浮按钮；点悬浮按钮直接开关取景框，不用先回主窗口再点一次。
//! - 退出只在悬浮按钮的右键菜单里。
//! - 取景框和悬浮按钮都浮在内容上面，必须排除出截图，否则会被识别进去。

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow};

use crate::error::CommandError;

pub const MAIN: &str = "main";
pub const LENS: &str = "lens";
pub const BUBBLE: &str = "bubble";

/// 悬浮按钮做成 136×56 的胶囊而不是圆点：实测 64×64 的无边框窗口会被 Windows 放宽到 135 宽
/// （默认最小跟踪宽度），加 minWidth、显示后 set_size 都缩不回去，索性用满这个宽度显示状态文字。
///
/// 悬浮按钮第一次出现时离屏幕右下角的距离（物理像素，按缩放比例换算前）。
const BUBBLE_MARGIN: (f64, f64) = (160.0, 180.0);

/// 悬浮按钮是否已经摆过位置。摆过之后不再动，保持用户拖到的地方。
static BUBBLE_PLACED: AtomicBool = AtomicBool::new(false);

fn window(app: &AppHandle, label: &str) -> Result<WebviewWindow, CommandError> {
    app.get_webview_window(label).ok_or_else(|| CommandError::new("window_missing", format!("窗口 {label} 不存在")))
}

/// 浮在内容上的窗口：排除出截图后再显示。
fn show_overlay(window: &WebviewWindow) -> Result<(), CommandError> {
    #[cfg(windows)]
    {
        let hwnd = window.hwnd().map_err(CommandError::internal)?;
        trans_win::exclude_from_capture(hwnd.0)
            .map_err(|error| CommandError::new("overlay_not_excluded", format!("无法把窗口排除出截图：{error}")))?;
    }
    window.show().map_err(CommandError::internal)
}

pub fn open_lens(app: &AppHandle) -> Result<(), CommandError> {
    let lens = window(app, LENS)?;
    show_overlay(&lens)?;
    lens.set_focus().map_err(CommandError::internal)
}

/// 只隐藏窗口，识别照常在后台进行；重新打开时面板里会有隐藏期间识别到的消息。
pub fn close_lens(app: &AppHandle) -> Result<(), CommandError> {
    window(app, LENS)?.hide().map_err(CommandError::internal)
}

/// 返回切换后取景框是否可见。
pub fn toggle_lens(app: &AppHandle) -> Result<bool, CommandError> {
    let visible = window(app, LENS)?.is_visible().map_err(CommandError::internal)?;
    if visible {
        close_lens(app)?;
    } else {
        open_lens(app)?;
    }
    Ok(!visible)
}

pub fn show_main(app: &AppHandle) -> Result<(), CommandError> {
    let main = window(app, MAIN)?;
    main.unminimize().map_err(CommandError::internal)?;
    main.show().map_err(CommandError::internal)?;
    main.set_focus().map_err(CommandError::internal)?;
    window(app, BUBBLE)?.hide().map_err(CommandError::internal)
}

/// 主窗口收起成悬浮按钮。按钮第一次出现放在屏幕右下角，之后保持用户拖到的位置。
pub fn collapse_to_bubble(app: &AppHandle) -> Result<(), CommandError> {
    window(app, MAIN)?.hide().map_err(CommandError::internal)?;
    let bubble = window(app, BUBBLE)?;
    if !BUBBLE_PLACED.swap(true, Ordering::Relaxed) {
        place_bottom_right(&bubble);
    }
    show_overlay(&bubble)
}

fn place_bottom_right(bubble: &WebviewWindow) {
    let Ok(Some(monitor)) = bubble.primary_monitor() else { return };
    let (size, origin, scale) = (monitor.size(), monitor.position(), monitor.scale_factor());
    let x = origin.x + size.width as i32 - (BUBBLE_MARGIN.0 * scale) as i32;
    let y = origin.y + size.height as i32 - (BUBBLE_MARGIN.1 * scale) as i32;
    if let Err(error) = bubble.set_position(PhysicalPosition::new(x, y)) {
        tracing::warn!(%error, "failed to place the floating button");
    }
}
