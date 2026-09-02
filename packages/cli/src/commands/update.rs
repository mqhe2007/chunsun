use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Args;
use serde::Deserialize;

use crate::commands::{CmdError, CmdResult};
use crate::harness::{
    refresh_installed_skill_templates, SkillTemplateRefreshResult,
};
use crate::loading::{format_bytes, LoadingIndicator, ProgressReporter};
use crate::{default_cli_download_url, load_config, resolve_api_base_url, ApiClient, version};

const REFRESH_TEMPLATES_COMMAND: &str = "_refresh-templates";

#[derive(Args)]
pub struct UpdateArgs {
    /// 仅检查是否有新版本，不下载安装
    #[arg(long)]
    check: bool,
}

#[derive(Debug, Deserialize)]
struct HealthInfo {
    version: String,
}

fn report_refresh(result: &SkillTemplateRefreshResult) {
    if !result.skill_installed {
        return;
    }
    if result.refreshed {
        let from = match &result.previous_version {
            Some(prev) => format!("{prev} → {}", result.template_version),
            None => result.template_version.clone(),
        };
        let ides: Vec<&str> = result.ides.iter().map(|id| id.as_str()).collect();
        println!(
            "[chunsun] 已自动刷新当前仓库技能模板（{from}），IDE：{}。",
            ides.join(" / ")
        );
    } else {
        println!(
            "[chunsun] 当前仓库技能模板已是最新（{}）。",
            result.template_version
        );
    }
}

/// 内部命令：升级后由「新」二进制执行，就地刷新当前仓库技能模板。
pub fn run_refresh_templates() -> CmdResult {
    let cwd = std::env::current_dir()?;
    let config = load_config();
    let api = ApiClient::new(&config)?;
    match refresh_installed_skill_templates(&api, &cwd) {
        Ok(result) => {
            report_refresh(&result);
            Ok(())
        }
        Err(err) => {
            eprintln!("[chunsun] 技能模板刷新失败：{err}");
            eprintln!("[chunsun] 可手动运行 `chunsun init` 刷新。");
            Err(CmdError::exit_only(1))
        }
    }
}

fn try_refresh_templates_from_instance(
    cwd: &Path,
) -> Result<Option<SkillTemplateRefreshResult>, CmdError> {
    let token = std::env::var("CHUNSUN_SECRET_KEY")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::env::var("CHUNSUN_TOKEN")
                .ok()
                .filter(|s| !s.is_empty())
        });
    let Some(token) = token else {
        eprintln!(
            "[chunsun] 未设置 CHUNSUN_SECRET_KEY，跳过技能模板刷新（可运行 `chunsun init`）。"
        );
        return Ok(None);
    };
    let config = crate::config::CliConfig {
        api_base_url: resolve_api_base_url(),
        token,
        project_id: std::env::var("_CHUNSUN_PROJECT_ID")
            .or_else(|_| std::env::var("CHUNSUN_PROJECT_ID"))
            .unwrap_or_default(),
    };
    let api = ApiClient::new(&config)?;
    let result = refresh_installed_skill_templates(&api, cwd)?;
    Ok(Some(result))
}

fn get_download_base_url() -> Result<String, CmdError> {
    if let Ok(override_url) = std::env::var("CHUNSUN_CLI_DOWNLOAD_URL") {
        let trimmed = override_url.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let embedded = default_cli_download_url().trim();
    if !embedded.is_empty() {
        return Ok(embedded.to_string());
    }

    derive_cli_url_from_api(&resolve_api_base_url())
}

/// 由 API 基址推导 CLI 下载目录（`/api/v1` → `/cli`）。
pub fn derive_cli_url_from_api(api_url: &str) -> Result<String, CmdError> {
    let trimmed = api_url.trim().trim_end_matches('/');
    if let Some(base) = trimmed.strip_suffix("/api/v1") {
        return Ok(format!("{base}/cli"));
    }
    Err(CmdError::new(format!(
        "无法从 API 地址推导 CLI 下载地址: {api_url}（期望以 /api/v1 结尾，或设置 CHUNSUN_CLI_DOWNLOAD_URL）"
    )))
}

fn fetch_instance_version(api_url: &str) -> Result<HealthInfo, CmdError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| CmdError::new(e.to_string()))?;
    let url = format!("{}/health", api_url.trim().trim_end_matches('/'));
    let res = client
        .get(&url)
        .send()
        .map_err(|e| CmdError::new(format!("查询实例版本失败: {e}")))?;
    if !res.status().is_success() {
        return Err(CmdError::new(format!(
            "查询实例版本失败 (HTTP {})",
            res.status().as_u16()
        )));
    }
    res.json::<HealthInfo>()
        .map_err(|e| CmdError::new(format!("解析实例版本失败: {e}")))
}

fn platform_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "windows",
        other => other,
    }
}

fn arch_name() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        other => other,
    }
}

fn get_binary_name() -> String {
    let arch = arch_name();
    if cfg!(windows) {
        format!("chunsun-cli-windows-{arch}.exe")
    } else {
        format!("chunsun-cli-{}-{arch}", platform_name())
    }
}

fn download_to(
    url: &str,
    dest_path: &Path,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<u64, CmdError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| CmdError::new(e.to_string()))?;
    let mut res = client
        .get(url)
        .send()
        .map_err(|e| CmdError::new(format!("下载失败: {e}")))?;
    if !res.status().is_success() {
        return Err(CmdError::new(format!(
            "下载失败 (HTTP {}): {url}",
            res.status().as_u16()
        )));
    }

    let total = res.content_length().filter(|&n| n > 0);
    let mut downloaded = 0u64;
    let mut file = fs::File::create(dest_path)?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = res
            .read(&mut buf)
            .map_err(|e| CmdError::new(format!("下载失败: {e}")))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;
        on_progress(downloaded, total);
    }
    on_progress(downloaded, total);
    Ok(downloaded)
}

/// 启动时清理 Windows 两步 rename 留下的 .old 残留文件。
/// 在其他平台为空操作。
pub fn cleanup_stale_update() {
    if !cfg!(windows) {
        return;
    }
    let Ok(exec) = std::env::current_exe() else {
        return;
    };
    let old_path = PathBuf::from(format!("{}.old", exec.display()));
    if old_path.exists() {
        let _ = fs::remove_file(&old_path);
    }
}

/// 解析标准 semver 版本号（x.y.z，可带前导 v）。
fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// 判断是否需要更新：latest 比 current 新时返回 true。
/// 两者均为标准 semver 时按元组比较；任一无法解析时回退为"不同即更新"（兼容旧行为）。
fn should_update(latest: &str, current: &str) -> bool {
    match (parse_semver(latest), parse_semver(current)) {
        (Some(l), Some(c)) => l > c,
        _ => latest != current,
    }
}

/// 安装后回读新二进制的真实版本号（运行 --version 并解析），
/// 避免直接回显 /health 的后端版本号而造成误导。
fn read_binary_version(path: &Path) -> Option<String> {
    let output = Command::new(path).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // 期望输出形如 "chunsun 0.4.8"，取最后一个空白分隔的 token
    stdout.split_whitespace().last().map(|s| s.to_string())
}

pub fn run(args: UpdateArgs) -> CmdResult {
    let api_url = resolve_api_base_url();
    let base_url = get_download_base_url()?;
    let current_version = version();

    if !args.check {
        println!("[chunsun] 当前版本 v{current_version}");
    }

    let checking = LoadingIndicator::start("[chunsun] 正在检查更新...");
    let latest = match fetch_instance_version(&api_url) {
        Ok(v) => {
            checking.stop();
            v
        }
        Err(err) => {
            checking.stop();
            eprintln!("[chunsun] 检查更新失败: {err}");
            return Err(CmdError::exit_only(1));
        }
    };

    if latest.version == current_version {
        if args.check {
            println!("[chunsun] 已是最新版本 v{current_version}");
            return Ok(());
        }
        println!("[chunsun] 已是最新版本 v{current_version}，无需更新");
        let cwd = std::env::current_dir()?;
        // 即使 CLI 二进制未变，也向实例核对模板版本（实例可能已发版新模板）。
        match try_refresh_templates_from_instance(&cwd) {
            Ok(Some(result)) => report_refresh(&result),
            Ok(None) => {}
            Err(err) => {
                eprintln!(
                    "[chunsun] 技能模板刷新失败：{err}，可手动运行 `chunsun init`。"
                );
            }
        }
        return Ok(());
    }

    if !should_update(&latest.version, current_version) {
        if args.check {
            println!(
                "[chunsun] 实例版本 v{} 不高于本地 CLI v{current_version}，无需更新",
                latest.version
            );
            return Ok(());
        }
        println!(
            "[chunsun] 实例版本 v{} 落后于本地 CLI v{current_version}，跳过自动更新（请先升级服务端实例）",
            latest.version
        );
        let cwd = std::env::current_dir()?;
        match try_refresh_templates_from_instance(&cwd) {
            Ok(Some(result)) => report_refresh(&result),
            Ok(None) => {}
            Err(err) => {
                eprintln!(
                    "[chunsun] 技能模板刷新失败：{err}，可手动运行 `chunsun init`。"
                );
            }
        }
        return Ok(());
    }

    if args.check {
        println!(
            "[chunsun] 发现新版本 v{}（当前 v{current_version}），请运行 chunsun update 更新",
            latest.version
        );
        return Err(CmdError::exit_only(1));
    }

    println!("[chunsun] 发现新版本 v{}", latest.version);

    let binary_name = get_binary_name();
    let download_url = format!("{base_url}/{binary_name}");
    let exec_path = std::env::current_exe().map_err(|e| CmdError::new(e.to_string()))?;
    let tmp_path = PathBuf::from(format!("{}.new", exec_path.display()));
    let mut progress = ProgressReporter::new("[chunsun] 下载中");

    let downloaded_bytes = match download_to(&download_url, &tmp_path, |done, total| {
        progress.update(done, total);
    }) {
        Ok(n) => {
            progress.succeed(&format!(
                "[chunsun] 下载完成（{}）",
                format_bytes(n)
            ));
            n
        }
        Err(err) => {
            progress.fail(&format!("[chunsun] 下载失败: {err}"));
            let _ = fs::remove_file(&tmp_path);
            return Err(CmdError::exit_only(1));
        }
    };
    let _ = downloaded_bytes;

    println!("[chunsun] 正在安装...");
    if let Err(err) = install_binary(&exec_path, &tmp_path) {
        eprintln!("[chunsun] 替换二进制失败: {err}");
        let _ = fs::remove_file(&tmp_path);
        return Err(CmdError::exit_only(1));
    }

    let real_new_version = read_binary_version(&exec_path).unwrap_or_else(|| latest.version.clone());
    println!(
        "[chunsun] 更新成功：v{current_version} → v{real_new_version}"
    );

    let refresh_ok = Command::new(&exec_path)
        .arg(REFRESH_TEMPLATES_COMMAND)
        // 新二进制若未内嵌默认 API，仍沿用本次 update 已连通的实例地址。
        .env("CHUNSUN_API_URL", &api_url)
        .current_dir(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !refresh_ok {
        println!("[chunsun] 若当前仓库技能未自动刷新，请运行 `chunsun init`");
    }
    println!(
        "[chunsun] 请重启终端使新版本生效；其它仓库进入后运行 `chunsun update` 或 `chunsun init` 即可刷新"
    );
    Ok(())
}

fn install_binary(exec_path: &Path, tmp_path: &Path) -> Result<(), CmdError> {
    if cfg!(windows) {
        let old_path = PathBuf::from(format!("{}.old", exec_path.display()));
        fs::rename(exec_path, &old_path)?;
        fs::rename(tmp_path, exec_path)?;
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(tmp_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(tmp_path, perms)?;
        }
        fs::rename(tmp_path, exec_path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_cli_url_strips_api_suffix() {
        assert_eq!(
            derive_cli_url_from_api("http://127.0.0.1:11111/api/v1").unwrap(),
            "http://127.0.0.1:11111/cli"
        );
        assert_eq!(
            derive_cli_url_from_api("https://chunsun.example.com/api/v1/").unwrap(),
            "https://chunsun.example.com/cli"
        );
    }

    #[test]
    fn derive_cli_url_rejects_unknown_shape() {
        assert!(derive_cli_url_from_api("https://example.com/v1").is_err());
    }

    #[test]
    fn parse_semver_handles_leading_v_and_plain() {
        assert_eq!(parse_semver("v0.4.8"), Some((0, 4, 8)));
        assert_eq!(parse_semver("0.4.8"), Some((0, 4, 8)));
        assert_eq!(parse_semver("  v1.2.3  "), Some((1, 2, 3)));
    }

    #[test]
    fn parse_semver_rejects_bad_input() {
        assert_eq!(parse_semver(""), None);
        assert_eq!(parse_semver("0.4"), None);
        assert_eq!(parse_semver("0.4.8.1"), None);
        assert_eq!(parse_semver("abc"), None);
        assert_eq!(parse_semver("v0.4.x"), None);
    }

    #[test]
    fn should_update_compares_by_semver() {
        // 更高 → 更新
        assert!(should_update("0.4.9", "0.4.8"));
        assert!(should_update("0.5.0", "0.4.9"));
        assert!(should_update("1.0.0", "0.9.9"));
        // 更低 → 不更新（阻止静默降级）
        assert!(!should_update("0.4.7", "0.4.8"));
        assert!(!should_update("0.3.9", "0.4.0"));
        // 相等 → 不更新（run() 中已提前处理，这里也应返回 false）
        assert!(!should_update("0.4.8", "0.4.8"));
    }

    #[test]
    fn should_update_fallback_when_unparseable() {
        // 任一无法解析时回退为"不同即更新"（兼容旧行为）
        assert!(should_update("custom-build", "0.4.8"));
        assert!(should_update("0.4.8", "custom-build"));
        assert!(!should_update("custom-build", "custom-build"));
    }
}
