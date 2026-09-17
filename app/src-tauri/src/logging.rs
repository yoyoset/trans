//! 应用日志：控制台一份（开发时看），日志目录下文件一份（发布版出问题时能拿到）。
//!
//! 日志目录要等 Tauri 起来才知道，所以文件输出是启动后再挂上的（`reload` 层）；
//! 挂上之前的少量启动日志只进控制台。
//!
//! 大小控制：每次启动把上一份改名为 `trans.previous.log`、新开 `trans.log`，最多留两份。
//! 应用只在状态变化、出错时记日志，单次运行的量很小，不做按大小切分。

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, reload, Layer, Registry};

const LOG_FILE: &str = "trans.log";
const PREVIOUS_LOG_FILE: &str = "trans.previous.log";

type FileLayer = Box<dyn Layer<Registry> + Send + Sync>;

pub struct Logging {
    file_layer: reload::Handle<Option<FileLayer>, Registry>,
}

impl Logging {
    /// 进程启动时调用一次。
    pub fn init() -> Self {
        let (file_layer, handle) = reload::Layer::new(None::<FileLayer>);
        // 级别过滤必须显式加：裸 registry 不过滤，reqwest/hyper 的 TRACE 连接日志会刷满文件
        // （实测未加时 4 分钟写了 1.8 万字节，几乎全是连接池日志）。
        tracing_subscriber::registry()
            .with(file_layer)
            .with(fmt::layer().with_target(false))
            .with(LevelFilter::INFO)
            .init();
        Self { file_layer: handle }
    }

    /// 日志目录确定后调用。失败只影响文件日志，应用照常运行。
    pub fn attach_file(&self, dir: &Path) -> io::Result<PathBuf> {
        let (path, file) = open_rotated(dir)?;
        let layer = fmt::layer().with_target(false).with_ansi(false).with_writer(Mutex::new(file)).boxed();
        self.file_layer.reload(Some(layer)).map_err(io::Error::other)?;
        Ok(path)
    }
}

/// 上一份日志改名保留，新开一份空日志。
fn open_rotated(dir: &Path) -> io::Result<(PathBuf, File)> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(LOG_FILE);
    if path.exists() {
        std::fs::rename(&path, dir.join(PREVIOUS_LOG_FILE))?;
    }
    let file = File::create(&path)?;
    Ok((path, file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_exactly_one_previous_log() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs");

        for run in 1..=3 {
            let (path, _) = open_rotated(&logs).unwrap();
            std::fs::write(&path, format!("run {run}")).unwrap();
        }

        assert_eq!(std::fs::read_to_string(logs.join(LOG_FILE)).unwrap(), "run 3");
        assert_eq!(std::fs::read_to_string(logs.join(PREVIOUS_LOG_FILE)).unwrap(), "run 2");
        assert_eq!(std::fs::read_dir(&logs).unwrap().count(), 2, "older logs are not accumulated");
    }
}
