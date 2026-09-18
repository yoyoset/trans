//! 记住取景框和悬浮按钮摆在哪、多大。
//!
//! 为什么不放进 `settings.json`：这是"上次用成什么样"的运行痕迹，不是用户配置，
//! 混在一起会让设置文件在每次挪动窗口后都变化。所以单独一个文件，坏了就当没有。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

/// 一个窗口的位置和大小，物理像素。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Geometry {
    /// 尺寸为 0 或负数的记录没有意义（最小化过、显示器拔掉等），当作无效丢弃。
    fn is_usable(&self) -> bool {
        self.width >= 64 && self.height >= 64
    }
}

/// 窗口位置文件。整份读写，窗口只有两三个，没必要做增量。
#[derive(Debug, Default)]
pub struct GeometryStore {
    path: PathBuf,
}

impl GeometryStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn read(&self) -> BTreeMap<String, Geometry> {
        std::fs::read_to_string(&self.path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
    }

    pub fn get(&self, label: &str) -> Option<Geometry> {
        self.read().get(label).copied().filter(Geometry::is_usable)
    }

    /// 记下当前窗口位置。窗口正在隐藏或读不到位置时什么都不做。
    pub fn remember(&self, window: &WebviewWindow) {
        let Ok(position) = window.outer_position() else { return };
        let Ok(size) = window.outer_size() else { return };
        let geometry = Geometry { x: position.x, y: position.y, width: size.width, height: size.height };
        if !geometry.is_usable() {
            return;
        }
        let mut all = self.read();
        if all.get(window.label()) == Some(&geometry) {
            return;
        }
        all.insert(window.label().to_owned(), geometry);
        if let Err(error) = write(&self.path, &all) {
            tracing::warn!(%error, "failed to remember window geometry");
        }
    }

    /// 还原窗口位置。没有记录就保持默认位置。
    pub fn restore(&self, window: &WebviewWindow, with_size: bool) {
        let Some(geometry) = self.get(window.label()) else { return };
        if with_size {
            let _ = window.set_size(PhysicalSize::new(geometry.width, geometry.height));
        }
        let _ = window.set_position(PhysicalPosition::new(geometry.x, geometry.y));
    }
}

fn write(path: &Path, all: &BTreeMap<String, Geometry>) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(all).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(dir: &Path) -> GeometryStore {
        GeometryStore::new(dir.join("windows.json"))
    }

    #[test]
    fn missing_or_broken_file_means_no_geometry() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(store(dir.path()).get("lens"), None);
        std::fs::write(dir.path().join("windows.json"), "{ not json").unwrap();
        assert_eq!(store(dir.path()).get("lens"), None);
    }

    #[test]
    fn reads_back_what_was_written_and_ignores_unusable_sizes() {
        let dir = tempfile::tempdir().unwrap();
        let all = BTreeMap::from([
            ("lens".to_owned(), Geometry { x: 100, y: 200, width: 960, height: 560 }),
            ("bubble".to_owned(), Geometry { x: 10, y: 20, width: 0, height: 0 }),
        ]);
        write(&dir.path().join("windows.json"), &all).unwrap();

        let store = store(dir.path());
        assert_eq!(store.get("lens"), Some(Geometry { x: 100, y: 200, width: 960, height: 560 }));
        assert_eq!(store.get("bubble"), None, "0 大小的记录不还原，否则窗口会消失");
    }
}
