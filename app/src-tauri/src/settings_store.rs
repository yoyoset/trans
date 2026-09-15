//! 设置文件的读写。格式与迁移逻辑在 `trans_application::settings`，这里只负责文件系统。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;
use trans_application::settings::{load_settings, AppSettings, SettingsError};

#[derive(Debug, Error)]
pub enum SettingsLoadError {
    #[error("读取设置文件失败：{0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Invalid(#[from] SettingsError),
}

pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 文件不存在是首次运行，返回默认设置；其他读取失败如实报错。
    pub fn load(&self) -> Result<AppSettings, SettingsLoadError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => Ok(load_settings(&text)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(AppSettings::default()),
            Err(error) => Err(error.into()),
        }
    }

    /// 先写临时文件再改名替换，写到一半断电也不会留下损坏的设置文件。
    pub fn save(&self, settings: &AppSettings) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &self.path)
    }

    /// 把读不了的设置文件改名备份，避免下次保存时覆盖掉用户可能想手动找回的内容。
    pub fn quarantine(&self) -> Option<PathBuf> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let backup = self.path.with_extension(format!("json.broken-{stamp}"));
        fs::rename(&self.path, &backup).ok().map(|()| backup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_means_first_run_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("settings.json"));
        assert_eq!(store.load().unwrap(), AppSettings::default());
    }

    #[test]
    fn save_then_load_round_trips_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("nested").join("settings.json"));
        let mut settings = AppSettings::default();
        settings.engines.local.threads = 12;
        settings.engines.local.model_path = Some(PathBuf::from(r"F:\hf_cache\Qwen3-4B-Instruct-2507-Q4_K_M.gguf"));
        store.save(&settings).unwrap();
        store.save(&settings).unwrap(); // 覆盖已有文件
        assert_eq!(store.load().unwrap(), settings);
        assert!(!store.path().with_extension("json.tmp").exists());
    }

    #[test]
    fn malformed_file_is_an_error_and_can_be_quarantined() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("settings.json"));
        fs::write(store.path(), "{ not json").unwrap();
        assert!(matches!(store.load(), Err(SettingsLoadError::Invalid(_))));
        let backup = store.quarantine().unwrap();
        assert!(backup.exists() && !store.path().exists());
    }
}
