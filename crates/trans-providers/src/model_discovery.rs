//! 本地 GGUF 模型发现。只读本地文件系统，绝不联网。
//!
//! 与其他项目共用 `HF_HOME`（本机为 `F:\hf_cache`）：
//! - subgen 把模型平铺在根目录；comic-gen 用 HF 缓存布局 `hub/models--*/snapshots/<hash>/`。
//! - 两种布局都要搜，搜索根必须包含 HF_HOME 根目录本身（comic-gen 踩过只搜 hub/ 的坑）。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// HF 缓存布局 root/hub/models--x--y/snapshots/<hash>/file.gguf 深度为 4，留一点余量。
const MAX_DEPTH: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelFile {
    pub path: PathBuf,
    pub file_name: String,
    pub size_bytes: u64,
}

pub fn search_roots(hf_home: Option<&str>, user_home: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(hf_home) = hf_home.map(str::trim).filter(|h| !h.is_empty()) {
        roots.push(PathBuf::from(hf_home));
    }
    if let Some(home) = user_home {
        roots.push(home.join(".cache").join("huggingface"));
    }
    roots
}

/// 同名同大小视为同一模型（平铺副本通常是 hub 里 blob 的硬链接），保留先找到的那份。
pub fn discover_gguf(roots: &[PathBuf]) -> Vec<ModelFile> {
    let mut found = Vec::new();
    for root in roots {
        walk(root, MAX_DEPTH, &mut found);
    }
    let mut seen = HashSet::new();
    found.retain(|m: &ModelFile| seen.insert((m.file_name.to_lowercase(), m.size_bytes)));
    found.sort_by(|a, b| a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()));
    found
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<ModelFile>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    // 先处理文件再进子目录，这样根目录的平铺副本优先于 hub 里的同名文件。
    entries.sort_by_key(|p| (p.is_dir(), p.clone()));
    for path in entries {
        if path.is_dir() {
            if depth > 0 {
                walk(&path, depth - 1, out);
            }
        } else if is_gguf(&path) {
            if let Ok(meta) = fs::metadata(&path) {
                let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                out.push(ModelFile { path, file_name, size_bytes: meta.len() });
            }
        }
    }
}

fn is_gguf(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0u8; bytes]).unwrap();
    }

    #[test]
    fn finds_flat_and_hub_layout_models_and_dedupes_copies() {
        let root = tempfile::tempdir().unwrap();
        let r = root.path();
        write(&r.join("Qwen3-4B-Instruct-2507-Q4_K_M.gguf"), 10);
        write(&r.join("hub/models--unsloth--Qwen3-4B/snapshots/abc123/Qwen3-4B-Instruct-2507-Q4_K_M.gguf"), 10);
        write(&r.join("hub/models--SakuraLLM--Sakura/snapshots/def456/sakura-7b.gguf"), 20);
        write(&r.join("ggml-large-v3.bin"), 5); // whisper 模型，不是 GGUF
        write(&r.join("hub/.locks/partial.gguf.incomplete"), 5); // 未下完

        let models = discover_gguf(&[r.to_path_buf()]);
        let names: Vec<_> = models.iter().map(|m| m.file_name.as_str()).collect();
        assert_eq!(names, ["Qwen3-4B-Instruct-2507-Q4_K_M.gguf", "sakura-7b.gguf"]);
        assert_eq!(models[0].path, r.join("Qwen3-4B-Instruct-2507-Q4_K_M.gguf"), "root flat copy wins");
    }

    #[test]
    fn missing_roots_are_skipped_silently() {
        assert!(discover_gguf(&[PathBuf::from(r"Z:\no\such\dir")]).is_empty());
    }

    #[test]
    fn search_roots_include_hf_home_itself_then_the_default_cache() {
        let roots = search_roots(Some(r"F:\hf_cache"), Some(Path::new(r"C:\Users\me")));
        assert_eq!(roots, [PathBuf::from(r"F:\hf_cache"), PathBuf::from(r"C:\Users\me\.cache\huggingface")]);
        assert_eq!(search_roots(Some("  "), None), Vec::<PathBuf>::new());
    }
}
