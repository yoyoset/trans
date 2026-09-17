//! 识别节奏：定时刷新，或者只在用户点「刷新」时才识别。
//!
//! 为什么要手动模式：用户在取景框旁边打中文时，输入法候选框是系统单独的窗口，排除不掉，
//! 一旦浮到取景区上就会被当成聊天内容识别进来。改成手动刷新，打字时就不会误读。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::Notify;

/// 界面允许的最短间隔。再短 CPU 会一直忙，而聊天消息也不会刷得这么快。
pub const MIN_INTERVAL: Duration = Duration::from_millis(500);
/// 默认间隔。M0-01 实测小区域 OCR 只要几十 ms，间隔主要是为了不让 CPU 一直忙。
pub const DEFAULT_INTERVAL: Duration = Duration::from_millis(1200);

/// 0 表示手动模式。
const MANUAL: u64 = 0;

pub struct RefreshControl {
    interval_ms: AtomicU64,
    refresh_requested: AtomicBool,
    wake: Notify,
}

impl Default for RefreshControl {
    fn default() -> Self {
        Self {
            interval_ms: AtomicU64::new(DEFAULT_INTERVAL.as_millis() as u64),
            refresh_requested: AtomicBool::new(false),
            wake: Notify::new(),
        }
    }
}

impl RefreshControl {
    /// `None` 是手动模式；定时间隔不足 [`MIN_INTERVAL`] 时按最短间隔算。
    pub fn interval(&self) -> Option<Duration> {
        match self.interval_ms.load(Ordering::Relaxed) {
            MANUAL => None,
            ms => Some(Duration::from_millis(ms)),
        }
    }

    /// 改节奏立即生效：正在等待的那一轮会按新间隔重新计算，不用等旧间隔走完。
    pub fn set_interval(&self, interval: Option<Duration>) {
        let ms = interval.map_or(MANUAL, |d| d.max(MIN_INTERVAL).as_millis() as u64);
        self.interval_ms.store(ms, Ordering::Relaxed);
        self.wake.notify_one();
    }

    /// 马上识别一轮，手动和定时模式都可用。
    pub fn request_refresh(&self) {
        self.refresh_requested.store(true, Ordering::Relaxed);
        self.wake.notify_one();
    }

    /// 等到下一轮该识别的时候。定时间隔从 `round_started` 算起，识别花掉的时间算在里面。
    pub async fn wait_next_round(&self, round_started: Instant) {
        loop {
            let notified = self.wake.notified();
            match self.interval() {
                None => notified.await,
                Some(interval) => {
                    tokio::select! {
                        _ = tokio::time::sleep(interval.saturating_sub(round_started.elapsed())) => return,
                        _ = notified => {}
                    }
                }
            }
            if self.refresh_requested.swap(false, Ordering::Relaxed) {
                return;
            }
            // 只是改了节奏：按新间隔继续等。
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHORT: Duration = Duration::from_millis(200);

    #[tokio::test]
    async fn manual_mode_waits_for_refresh() {
        let control = RefreshControl::default();
        control.set_interval(None);
        let waited = tokio::time::timeout(SHORT, control.wait_next_round(Instant::now())).await;
        assert!(waited.is_err(), "手动模式不应自己开始下一轮");

        control.request_refresh();
        tokio::time::timeout(SHORT, control.wait_next_round(Instant::now())).await.expect("点了刷新应立即开始");
    }

    #[tokio::test]
    async fn switching_to_manual_does_not_trigger_a_round() {
        let control = RefreshControl::default();
        control.set_interval(None);
        let waited = tokio::time::timeout(SHORT, control.wait_next_round(Instant::now())).await;
        assert!(waited.is_err(), "切换节奏本身不算一次刷新");
    }

    #[tokio::test]
    async fn timed_mode_returns_after_interval() {
        let control = RefreshControl::default();
        control.set_interval(Some(MIN_INTERVAL));
        let started = Instant::now();
        control.wait_next_round(started).await;
        assert!(started.elapsed() >= MIN_INTERVAL);
    }

    #[test]
    fn interval_is_clamped_to_minimum() {
        let control = RefreshControl::default();
        control.set_interval(Some(Duration::from_millis(10)));
        assert_eq!(control.interval(), Some(MIN_INTERVAL));
    }
}
