//! 读写 chunsun.json 中的实例字段（热更新内存 + 落盘）。

use crate::api::AppError;
use crate::instance::{self, InstanceFile};
use crate::state::AppState;

pub fn update_public_origin(state: &AppState, raw: &str) -> Result<(), AppError> {
    let origin = normalize_origin(raw)?;
    state.with_ready_config(|cfg| {
        cfg.public_origin = origin.clone();
        cfg.node_env = if origin.starts_with("https://") {
            Some("production".into())
        } else {
            None
        };
        Ok(())
    }).map_err(|e| AppError::internal(e))?;
    let file = InstanceFile::from_config(&state.config());
    instance::save_file(state.config_path(), &file)
        .map_err(|e| AppError::internal(format!("写入实例配置失败: {e}")))?;
    Ok(())
}

fn normalize_origin(raw: &str) -> Result<String, AppError> {
    let origin = raw.trim().trim_end_matches('/').to_string();
    if origin.is_empty() {
        return Err(AppError::unprocessable("VALIDATION_ERROR").with_message("站点地址不能为空"));
    }
    if !(origin.starts_with("http://") || origin.starts_with("https://")) {
        return Err(AppError::unprocessable("VALIDATION_ERROR")
            .with_message("站点地址需以 http:// 或 https:// 开头"));
    }
    Ok(origin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_trailing_slash() {
        assert_eq!(
            normalize_origin("https://example.com/").unwrap(),
            "https://example.com"
        );
    }

    #[test]
    fn normalize_rejects_non_http() {
        assert!(normalize_origin("ftp://x.com").is_err());
    }
}
