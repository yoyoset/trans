//! 真机：写剪贴板、把内容粘贴到指定屏幕坐标所在的窗口。
//!
//! 默认忽略（要在屏幕上先摆一个可输入的窗口）。运行方式（PowerShell）：
//! ```text
//! # 先起一个测试窗口：scratchpad\paste_target.ps1
//! $env:TRANS_PASTE_POINT = '500,700'   # 屏幕坐标，落在那个输入框里
//! cargo test -p trans-win --test send_text_real -- --ignored --nocapture
//! ```

#![cfg(windows)]

#[test]
#[ignore = "needs a text box on screen at TRANS_PASTE_POINT"]
fn pastes_into_the_window_under_the_point() {
    let raw = std::env::var("TRANS_PASTE_POINT").expect("set TRANS_PASTE_POINT to x,y inside a text box");
    let (x, y) = raw.split_once(',').expect("TRANS_PASTE_POINT is x,y");
    let (x, y) = (x.trim().parse().expect("x"), y.trim().parse().expect("y"));

    let text = "trans 粘贴测试 42";
    trans_win::set_clipboard_text(text).expect("clipboard write works");
    match trans_win::paste_at(x, y, false) {
        Ok(()) => println!("pasted {text:?} at {x},{y}"),
        // 从 cargo test 里跑时本进程不在前台，系统会挡住前台切换：这时必须报错而不是乱发按键。
        Err(error) => {
            println!("refused to paste: {error}");
            assert!(error.contains("没能切换到目标窗口"), "unexpected error: {error}");
        }
    }
}
