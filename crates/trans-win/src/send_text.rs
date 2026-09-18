//! 把译文送回聊天窗口：写剪贴板，并按需要替用户粘贴（Ctrl+V，可选回车）。
//!
//! 只在用户明确点击时调用。粘贴目标是取景框框住的那个窗口——框在哪就发到哪，
//! 不需要用户再指认一次输入框。游戏场景默认只复制不模拟按键（反作弊风险未实测）。

use std::time::Duration;

use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND, POINT};
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, SetFocus, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL,
    VK_RETURN, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetAncestor, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow, ShowWindow,
    WindowFromPoint, GA_ROOT, SW_SHOW,
};

/// 切换前台窗口后等一下再发按键：窗口需要时间完成激活，立刻发按键会打到旧窗口上。
const FOCUS_SETTLE: Duration = Duration::from_millis(60);

/// 等目标窗口成为前台窗口的上限。
const FOCUS_TIMEOUT: Duration = Duration::from_millis(600);

/// 写入剪贴板（Unicode 文本）。
pub fn set_clipboard_text(text: &str) -> Result<(), String> {
    let mut utf16: Vec<u16> = text.encode_utf16().collect();
    utf16.push(0);
    let bytes = std::mem::size_of_val(&utf16[..]);

    // SAFETY: 全程按 Win32 剪贴板约定调用：打开 → 清空 → 写入 → 关闭；
    // 内存块交给系统后不再由我们释放（SetClipboardData 成功时所有权转移）。
    unsafe {
        OpenClipboard(None).map_err(|error| format!("打不开剪贴板：{error}"))?;
        let result = (|| {
            EmptyClipboard().map_err(|error| format!("清空剪贴板失败：{error}"))?;
            let handle: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(|e| format!("分配剪贴板内存失败：{e}"))?;
            let target = GlobalLock(handle);
            if target.is_null() {
                return Err("锁定剪贴板内存失败".to_owned());
            }
            std::ptr::copy_nonoverlapping(utf16.as_ptr(), target.cast::<u16>(), utf16.len());
            let _ = GlobalUnlock(handle);
            SetClipboardData(CF_UNICODETEXT.0.into(), Some(HANDLE(handle.0)))
                .map(|_| ())
                .map_err(|error| format!("写入剪贴板失败：{error}"))
        })();
        let _ = CloseClipboard();
        result
    }
}

/// 把文字粘贴到屏幕上某个点所在的窗口。`press_enter` 为真时再按一次回车发送。
pub fn paste_at(x: i32, y: i32, press_enter: bool) -> Result<(), String> {
    let point = POINT { x, y };
    // SAFETY: 只读系统状态；WindowFromPoint 可能返回空窗口，下面会判。
    let window = unsafe { WindowFromPoint(point) };
    if window.is_invalid() {
        return Err("这个位置上没有窗口".into());
    }
    // SAFETY: window 有效。
    let root = unsafe { GetAncestor(window, GA_ROOT) };
    let target = if root.is_invalid() { window } else { root };

    focus(target, window);
    // 切不过去就绝不发按键：实测前台切换失败时，Ctrl+V 会打进当时的前台窗口（测试里打进了 Chrome）。
    std::thread::sleep(FOCUS_SETTLE);
    if !wait_until_foreground(target) {
        return Err("没能切换到目标窗口（可能被系统的前台切换限制挡住），已复制到剪贴板，请自己粘贴".into());
    }

    send_chord(VK_V, true)?;
    if press_enter {
        std::thread::sleep(Duration::from_millis(60));
        send_chord(VK_RETURN, false)?;
    }
    Ok(())
}

/// 等目标窗口真的变成前台窗口，最多等 [`FOCUS_TIMEOUT`]。
fn wait_until_foreground(target: HWND) -> bool {
    let deadline = std::time::Instant::now() + FOCUS_TIMEOUT;
    loop {
        // SAFETY: 只读系统状态。
        if unsafe { GetForegroundWindow() } == target {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// 切到目标窗口。Windows 默认不许后台进程抢前台，所以先把本线程的输入状态挂到当前前台线程上，
/// 这是切前台窗口的常规做法（实测不这么做时按键会打在原来的窗口上，目标输入框一个字也收不到）。
fn focus(target: HWND, child: HWND) {
    // SAFETY: 句柄来自 WindowFromPoint/GetAncestor；Attach 与 Detach 成对调用。
    unsafe {
        let foreground = GetForegroundWindow();
        let foreground_thread = GetWindowThreadProcessId(foreground, None);
        let current_thread = GetCurrentThreadId();
        let attached = foreground_thread != 0
            && foreground_thread != current_thread
            && AttachThreadInput(current_thread, foreground_thread, true).as_bool();

        let _ = ShowWindow(target, SW_SHOW);
        let _ = BringWindowToTop(target);
        let _ = SetForegroundWindow(target);
        let _ = SetFocus(Some(child));

        if attached {
            let _ = AttachThreadInput(current_thread, foreground_thread, false);
        }
    }
}

/// 发送一次按键；`with_ctrl` 为真时带住 Ctrl。
fn send_chord(key: VIRTUAL_KEY, with_ctrl: bool) -> Result<(), String> {
    let mut inputs = Vec::new();
    if with_ctrl {
        inputs.push(key_input(VK_CONTROL, false));
    }
    inputs.push(key_input(key, false));
    inputs.push(key_input(key, true));
    if with_ctrl {
        inputs.push(key_input(VK_CONTROL, true));
    }
    // SAFETY: inputs 是本地数组，长度与结构体大小按 API 要求传入。
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err("模拟按键被系统拦截（可能目标程序以管理员身份运行）".into())
    }
}

fn key_input(key: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                ..Default::default()
            },
        },
    }
}
