use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("packages/cli should live under monorepo root")
        .to_path_buf();

    println!("cargo:rerun-if-changed={}", repo_root.join("package.json").display());
    println!(
        "cargo:rerun-if-env-changed=CHUNSUN_DEFAULT_API_URL"
    );
    println!("cargo:rerun-if-env-changed=CHUNSUN_DEFAULT_CLI_DOWNLOAD_URL");
    println!("cargo:rerun-if-changed={}", repo_root.join(".env").display());

    let version = read_root_version(&repo_root);
    println!("cargo:rustc-env=CHUNSUN_VERSION={version}");

    let (api_url, download_url) = resolve_defaults(&repo_root);
    println!("cargo:rustc-env=CHUNSUN_DEFAULT_API_URL={api_url}");
    println!("cargo:rustc-env=CHUNSUN_DEFAULT_CLI_DOWNLOAD_URL={download_url}");
}

fn read_root_version(repo_root: &std::path::Path) -> String {
    let pkg = fs::read_to_string(repo_root.join("package.json")).unwrap_or_default();
    // Minimal parse: "version": "x.y.z"
    for line in pkg.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("\"version\"") {
            if let Some(start) = rest.find('"') {
                let after = &rest[start + 1..];
                if let Some(end) = after.find('"') {
                    return after[..end].to_string();
                }
            }
        }
    }
    "0.0.0".to_string()
}

fn resolve_defaults(repo_root: &std::path::Path) -> (String, String) {
    if let Ok(api) = env::var("CHUNSUN_DEFAULT_API_URL") {
        let download = env::var("CHUNSUN_DEFAULT_CLI_DOWNLOAD_URL").unwrap_or_default();
        if !api.trim().is_empty() {
            return (api.trim().to_string(), download.trim().to_string());
        }
    }

    // Fall back to .env PUBLIC_ORIGIN / HOST_BASE_PATH
    let mut public_origin = String::new();
    let mut host_base_path = "/".to_string();
    if let Ok(content) = fs::read_to_string(repo_root.join(".env")) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim();
                let v = v.trim().trim_matches('"');
                match k {
                    "PUBLIC_ORIGIN" => public_origin = v.trim_end_matches('/').to_string(),
                    "HOST_BASE_PATH" => host_base_path = normalize_base_path(v),
                    _ => {}
                }
            }
        }
    }

    if public_origin.is_empty() {
        return (String::new(), String::new());
    }
    (
        join_public_url(&public_origin, &host_base_path, "api/v1"),
        join_public_url(&public_origin, &host_base_path, "cli"),
    )
}

fn normalize_base_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }
    let with_leading = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    if with_leading.ends_with('/') {
        with_leading
    } else {
        format!("{with_leading}/")
    }
}

fn join_public_url(origin: &str, base_path: &str, segment: &str) -> String {
    let origin_clean = origin.trim_end_matches('/');
    let path = if base_path == "/" {
        format!("/{segment}")
    } else {
        format!("{base_path}{segment}")
    };
    format!("{origin_clean}{path}")
}
