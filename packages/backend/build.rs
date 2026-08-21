//! 把官网 / 控制台 dist 与 CLI 产物拷到 OUT_DIR，供 rust-embed 编译进二进制。
//! 目录缺失时写入占位文件，保证 `cargo test` 在未先构建前端时也能通过。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let repo = manifest.parent().and_then(|p| p.parent()).unwrap();

    let web_out = out.join("web");
    let cli_scripts_out = out.join("cli-scripts");
    let cli_dist_out = out.join("cli-dist");

    fs::create_dir_all(&web_out).unwrap();

    let website_dist = repo.join("packages/website/dist");
    if website_dist.join("index.html").is_file() {
        copy_dir(&website_dist, &web_out).expect("copy website dist");
    }

    let console_dist = repo.join("packages/console/dist");
    let console_out = web_out.join("console");
    if console_dist.join("index.html").is_file() {
        copy_dir(&console_dist, &console_out).expect("copy console dist");
    }

    if !web_out.join("index.html").is_file() {
        fs::write(
            web_out.join("index.html"),
            b"<!doctype html><meta charset=\"utf-8\"><title>chunsun</title><p>website dist missing</p>\n",
        )
        .unwrap();
    }
    if !console_out.join("index.html").is_file() {
        fs::create_dir_all(&console_out).unwrap();
        fs::write(
            console_out.join("index.html"),
            b"<!doctype html><meta charset=\"utf-8\"><title>chunsun</title><p>console dist missing</p>\n",
        )
        .unwrap();
    }

    copy_dir(&repo.join("packages/cli/scripts"), &cli_scripts_out).expect("copy cli scripts");

    let cli_dist = repo.join("packages/cli/dist");
    if cli_dist.is_dir() {
        copy_dir(&cli_dist, &cli_dist_out).expect("copy cli dist");
    }
    fs::create_dir_all(&cli_dist_out).unwrap();
    if dir_is_empty(&cli_dist_out) {
        fs::write(cli_dist_out.join(".keep"), b"").unwrap();
    }

    println!("cargo:rerun-if-changed={}", website_dist.display());
    println!("cargo:rerun-if-changed={}", console_dist.display());
    println!(
        "cargo:rerun-if-changed={}",
        repo.join("packages/cli/scripts").display()
    );
    println!("cargo:rerun-if-changed={}", cli_dist.display());
}

fn dir_is_empty(path: &Path) -> bool {
    fs::read_dir(path)
        .map(|mut it| it.next().is_none())
        .unwrap_or(true)
}

fn copy_dir(src: &Path, dst: &Path) -> io::Result<()> {
    if !src.is_dir() {
        fs::create_dir_all(dst)?;
        return Ok(());
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else if ty.is_file() {
            fs::copy(&entry.path(), to)?;
        }
    }
    Ok(())
}
