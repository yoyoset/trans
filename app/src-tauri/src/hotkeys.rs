//! 全局热键：不用先找窗口，按键就能呼出取景框、让取景区不挡鼠标、手动读一次。
//!
//! 热键是系统级的，注册失败（多半是被别的软件占了）只记日志并作为启动提示告诉用户，
//! 不影响其它功能——界面上的按钮一直都在。

use std::sync::{Mutex, PoisonError};

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use trans_application::settings::HotkeySettings;

use crate::overlay_windows;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// 打开/收起取景框。
    ToggleLens,
    /// 取景框让开鼠标（点击穿到底下的窗口）/ 恢复。
    ToggleClickThrough,
    /// 立即识别一轮。
    RefreshNow,
}

impl Action {
    fn describe(self) -> &'static str {
        match self {
            Self::ToggleLens => "打开/收起取景框",
            Self::ToggleClickThrough => "取景框让开鼠标 / 恢复",
            Self::RefreshNow => "立即识别一次",
        }
    }
}

fn bindings(settings: &HotkeySettings) -> Vec<(String, Action)> {
    [
        (settings.toggle_lens.trim(), Action::ToggleLens),
        (settings.click_through.trim(), Action::ToggleClickThrough),
        (settings.refresh.trim(), Action::RefreshNow),
    ]
    .into_iter()
    .filter(|(keys, _)| !keys.is_empty())
    .map(|(keys, action)| (keys.to_owned(), action))
    .collect()
}

/// 界面上给用户看的说明：当前真正生效的热键各自做什么。
pub fn descriptions() -> Vec<(String, &'static str)> {
    REGISTERED.lock().unwrap_or_else(PoisonError::into_inner).clone().unwrap_or_default()
}

/// 当前生效的热键。设置保存后重新注册时更新。
static REGISTERED: Mutex<Option<Vec<(String, &'static str)>>> = Mutex::new(None);

/// 按设置注册热键（先清掉旧的），返回注册失败的说明。
pub fn apply(app: &AppHandle, settings: &HotkeySettings) -> Vec<String> {
    if let Err(error) = app.global_shortcut().unregister_all() {
        tracing::warn!(%error, "failed to clear old hotkeys");
    }
    let mut problems = Vec::new();
    let mut live = Vec::new();
    for (keys, action) in bindings(settings) {
        let shortcut: Shortcut = match keys.parse() {
            Ok(shortcut) => shortcut,
            Err(error) => {
                tracing::warn!(keys, %error, "hotkey not parsed");
                problems.push(format!("热键「{keys}」写法不对：{error}"));
                continue;
            }
        };
        let result = app.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, event| {
            // 只在按下时触发；松开也会回调一次。
            if event.state() == ShortcutState::Pressed {
                run(app, action);
            }
        });
        match result {
            Ok(()) => {
                tracing::info!(keys, ?action, "hotkey registered");
                live.push((keys, action.describe()));
            }
            Err(error) => {
                tracing::warn!(keys, %error, "hotkey not registered");
                problems.push(format!("热键「{keys}」注册失败，多半被别的软件占用了，请在设置里换一个：{error}"));
            }
        }
    }
    *REGISTERED.lock().unwrap_or_else(PoisonError::into_inner) = Some(live);
    problems
}

fn run(app: &AppHandle, action: Action) {
    tracing::info!(?action, "hotkey pressed");
    let result = match action {
        Action::ToggleLens => overlay_windows::toggle_lens(app).map(|_| ()),
        Action::ToggleClickThrough => overlay_windows::toggle_click_through(app).map(|_| ()),
        Action::RefreshNow => {
            app.state::<AppState>().capture.refresh_if_running();
            Ok(())
        }
    };
    if let Err(error) = result {
        tracing::warn!(?error, ?action, "hotkey action failed");
    }
}
