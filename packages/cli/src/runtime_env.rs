use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

/// 未显式设置 NODE_ENV 时：cargo run 开发用 development，否则 production。
pub fn resolve_runtime_node_env(argv: &[String]) -> String {
    if let Ok(explicit) = std::env::var("NODE_ENV") {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return trimmed.to_lowercase();
        }
    }

    // 开发：通过 `cargo run` 时 CARGO 环境变量存在
    if std::env::var_os("CARGO").is_some() {
        return "development".into();
    }

    if let Some(entry) = argv.get(1) {
        let normalized = Path::new(entry);
        if normalized.ends_with(Path::new("src/main.rs"))
            || normalized.ends_with(Path::new("src/index.ts"))
        {
            return "development".into();
        }
    }

    "production".into()
}

/** 判断当前进程是否已有该环境变量（含空字符串） */
pub fn has_process_env_key(key: &str) -> bool {
    std::env::vars_os().any(|(k, _)| k == key)
}

pub fn resolve_dotenv_candidate_paths(cwd: &Path, argv: &[String]) -> Vec<PathBuf> {
    if let Ok(explicit) = std::env::var("CHUNSUN_ENV_FILE") {
        return vec![PathBuf::from(explicit)];
    }
    let node_env = resolve_runtime_node_env(argv);
    vec![
        cwd.join(".env.local"),
        cwd.join(format!(".env.{node_env}.local")),
        cwd.join(format!(".env.{node_env}")),
        cwd.join(".env"),
    ]
}

pub fn list_local_dotenv_keys(cwd: Option<&Path>, argv: Option<&[String]>) -> Vec<String> {
    let cwd = cwd
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let owned_argv: Vec<String> = argv
        .map(|a| a.to_vec())
        .unwrap_or_else(|| std::env::args().collect());
    let mut keys = BTreeSet::new();
    for file_path in resolve_dotenv_candidate_paths(&cwd, &owned_argv) {
        if !file_path.is_file() {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&file_path) else {
            continue;
        };
        for item in dotenvy::from_read_iter(raw.as_bytes()) {
            if let Ok((k, _)) = item {
                if !k.is_empty() {
                    keys.insert(k);
                }
            }
        }
    }
    keys.into_iter().collect()
}

fn try_load_file(file_path: &Path) -> Result<bool> {
    if !file_path.is_file() {
        return Ok(false);
    }
    match dotenvy::from_path(file_path) {
        Ok(()) => Ok(true),
        Err(e) => Err(anyhow!(
            "读取环境配置文件失败（{}）：{e}",
            file_path.display()
        )),
    }
}

/// 按优先级加载环境变量（shell 已有变量不被覆盖）。
/// dotenvy::from_path 默认不覆盖已有环境变量。
pub fn load_runtime_env(argv: Option<&[String]>) -> Result<Option<PathBuf>> {
    let owned_argv: Vec<String> = argv
        .map(|a| a.to_vec())
        .unwrap_or_else(|| std::env::args().collect());

    if let Ok(explicit) = std::env::var("CHUNSUN_ENV_FILE") {
        let resolved = PathBuf::from(&explicit);
        try_load_file(&resolved)?;
        return Ok(Some(resolved));
    }

    let node_env = resolve_runtime_node_env(&owned_argv);
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let candidates = [
        cwd.join(".env.local"),
        cwd.join(format!(".env.{node_env}.local")),
        cwd.join(format!(".env.{node_env}")),
        cwd.join(".env"),
    ];

    let mut primary: Option<PathBuf> = None;
    for file_path in &candidates {
        if try_load_file(file_path)? && primary.is_none() {
            primary = Some(file_path.clone());
        }
    }
    Ok(primary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn has_process_env_key_detects_empty() {
        std::env::set_var("CHUNSUN_TEST_EMPTY", "");
        assert!(has_process_env_key("CHUNSUN_TEST_EMPTY"));
        std::env::remove_var("CHUNSUN_TEST_EMPTY");
        assert!(!has_process_env_key("CHUNSUN_TEST_EMPTY"));
    }

    #[test]
    fn list_local_dotenv_keys_reads_files() {
        let dir = tempdir().unwrap();
        let env_path = dir.path().join(".env");
        let mut f = fs::File::create(&env_path).unwrap();
        writeln!(f, "FOO=1\nBAR=2").unwrap();
        let keys = list_local_dotenv_keys(Some(dir.path()), Some(&[]));
        assert!(keys.contains(&"FOO".to_string()));
        assert!(keys.contains(&"BAR".to_string()));
    }
}
