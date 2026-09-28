use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;

use crate::config::CliConfig;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{0}")]
    Message(String),
    #[error("请求超时：{method} {path}（10 秒内无响应）")]
    Timeout { method: String, path: String },
    #[error("{0}")]
    Network(#[from] reqwest::Error),
    /// 非 2xx 响应，**保留完整错误包络**。
    ///
    /// `Display` 刻意与 `Message` 分支逐字节一致（都是 `"{method} {path} 失败 ({status}): {detail}"`），
    /// 因为 `commands/harness.rs` 靠 `to_string().contains("RUN_ALREADY_RUNNING")` 判撞锁——
    /// 换掉文案会静默打断那条分支。`code` / `body` 是给新代码用的**增量**能力：
    /// `conflict.rs` 需要拿 409 里的 `data.currentContent` 才能做合并提示，
    /// 而这些字段原先在 `parse_error_detail` 里就被丢掉了。
    #[error("{method} {path} 失败 ({status}): {detail}")]
    Http {
        method: String,
        path: String,
        status: u16,
        detail: String,
        code: Option<String>,
        /// 原始响应体。不预先解析：大多数调用点只看 `code`，而 409 可能带整篇正文。
        body: String,
    },
}

impl ApiError {
    /// 业务错误码（响应体顶层 `error` 字段）。非 `Http` 变体或响应不是 JSON 时为 `None`。
    pub fn code(&self) -> Option<&str> {
        match self {
            ApiError::Http { code, .. } => code.as_deref(),
            _ => None,
        }
    }

    /// 原始响应体解析成 JSON。非 `Http` 变体、或响应不是 JSON 时为 `None`。
    pub fn body(&self) -> Option<Value> {
        match self {
            ApiError::Http { body, .. } => serde_json::from_str(body).ok(),
            _ => None,
        }
    }
}

pub struct ApiClient {
    base_url: String,
    token: String,
    client: reqwest::blocking::Client,
}

impl ApiClient {
    pub fn new(config: &CliConfig) -> Result<Self, ApiError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent(format!("chunsun-cli/{}", crate::version()))
            .build()?;
        Ok(Self {
            base_url: config.api_base_url.clone(),
            token: config.token.clone(),
            client,
        })
    }

    fn request<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<T, ApiError> {
        let url = format!("{}{path}", self.base_url);
        let mut builder = match method {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PATCH" => self.client.patch(&url),
            "PUT" => self.client.put(&url),
            "DELETE" => self.client.delete(&url),
            _ => {
                return Err(ApiError::Message(format!("不支持的 HTTP 方法：{method}")));
            }
        };
        builder = builder
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.token));
        if let Some(b) = body {
            builder = builder.json(b);
        }

        let res = match builder.send() {
            Ok(r) => r,
            Err(e) if e.is_timeout() => {
                return Err(ApiError::Timeout {
                    method: method.to_string(),
                    path: path.to_string(),
                });
            }
            Err(e) => return Err(ApiError::Network(e)),
        };

        if !res.status().is_success() {
            let status = res.status().as_u16();
            let text = res.text().unwrap_or_default();
            let detail = parse_error_detail(&text);
            return Err(ApiError::Http {
                method: method.to_string(),
                path: path.to_string(),
                status,
                detail,
                code: parse_error_code(&text),
                body: text,
            });
        }

        res.json::<T>()
            .map_err(|e| ApiError::Message(format!("解析响应失败：{e}")))
    }

    pub fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        self.request("GET", path, None)
    }

    pub fn post<T: DeserializeOwned>(&self, path: &str, body: Value) -> Result<T, ApiError> {
        self.request("POST", path, Some(&body))
    }

    pub fn patch<T: DeserializeOwned>(&self, path: &str, body: Value) -> Result<T, ApiError> {
        self.request("PATCH", path, Some(&body))
    }

    pub fn put<T: DeserializeOwned>(&self, path: &str, body: Value) -> Result<T, ApiError> {
        self.request("PUT", path, Some(&body))
    }

    pub fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        self.request("DELETE", path, None)
    }
}

/// 只取顶层 `error` 字段，给按错误码分流的调用方用。
///
/// 与 `parse_error_detail` 分开是有意的：后者会把 `GATE_BLOCKED` 的 `data.hard`
/// 展开成一长串人读文案，那样得到的字符串不再是错误码。
fn parse_error_code(text: &str) -> Option<String> {
    let parsed = serde_json::from_str::<Value>(text).ok()?;
    Some(parsed.get("error")?.as_str()?.to_string())
}

fn parse_error_detail(text: &str) -> String {
    let Ok(parsed) = serde_json::from_str::<Value>(text) else {
        return text.to_string();
    };
    if parsed.get("error").and_then(|v| v.as_str()) == Some("GATE_BLOCKED") {
        if let Some(hard) = parsed
            .pointer("/data/hard")
            .and_then(|v| v.as_array())
        {
            let lines: Vec<String> = hard
                .iter()
                .filter_map(|g| {
                    let id = g.get("id")?.as_str()?;
                    let message = g.get("message")?.as_str()?;
                    let hint = g
                        .get("hint")
                        .and_then(|h| h.as_str())
                        .map(|h| format!("（{h}）"))
                        .unwrap_or_default();
                    Some(format!("{id} {message}{hint}"))
                })
                .collect();
            if !lines.is_empty() {
                return format!("GATE_BLOCKED: {}", lines.join("; "));
            }
        }
    }
    if let Some(err) = parsed.get("error").and_then(|v| v.as_str()) {
        return err.to_string();
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFLICT_BODY: &str = r#"{
        "success": false,
        "error": "KNOWLEDGE_DOC_CONFLICT",
        "message": "文档已被他人修改",
        "hint": "用 currentContent 合并后再写",
        "data": {
            "docId": "d1",
            "yourRevision": 7,
            "currentRevision": 9,
            "currentContent": "最新正文"
        }
    }"#;

    fn http_error(text: &str) -> ApiError {
        ApiError::Http {
            method: "PUT".into(),
            path: "/api/v1/projects/p1/knowledge/documents/d1".into(),
            status: 409,
            detail: parse_error_detail(text),
            code: parse_error_code(text),
            body: text.to_string(),
        }
    }

    /// 回归防线：`commands/harness.rs` 用 `contains("RUN_ALREADY_RUNNING")` 判撞锁，
    /// `Display` 必须继续把错误码原样带出来。
    #[test]
    fn display_still_contains_the_business_code() {
        let err = http_error(r#"{"error":"RUN_ALREADY_RUNNING"}"#);
        assert!(err.to_string().contains("RUN_ALREADY_RUNNING"));
        assert!(err.to_string().starts_with("PUT /api/v1/projects/p1/knowledge/documents/d1 失败 (409): "));
    }

    #[test]
    fn code_exposes_the_top_level_error_field() {
        let err = http_error(CONFLICT_BODY);
        assert_eq!(err.code(), Some("KNOWLEDGE_DOC_CONFLICT"));
    }

    #[test]
    fn body_exposes_structured_data_that_detail_drops() {
        let err = http_error(CONFLICT_BODY);
        let body = err.body().expect("响应体是 JSON");
        assert_eq!(body.pointer("/data/currentRevision").and_then(|v| v.as_i64()), Some(9));
        assert_eq!(
            body.pointer("/data/currentContent").and_then(|v| v.as_str()),
            Some("最新正文")
        );
    }

    /// `detail` 仍只给错误码（旧行为），合并所需的信息只从 `body()` 走——
    /// 两条路径不要互相污染。
    #[test]
    fn detail_stays_a_bare_code_while_body_carries_the_rest() {
        let err = http_error(CONFLICT_BODY);
        assert_eq!(err.code(), Some("KNOWLEDGE_DOC_CONFLICT"));
        assert!(!err.to_string().contains("currentContent"));
    }

    #[test]
    fn non_json_body_degrades_without_panicking() {
        let err = http_error("<html>502 Bad Gateway</html>");
        assert_eq!(err.code(), None);
        assert!(err.body().is_none());
        assert!(err.to_string().contains("<html>502 Bad Gateway</html>"));
    }

    /// `GATE_BLOCKED` 的 `data.hard` 展开仍要工作；但 `code()` 取的是顶层字段本身，
    /// 不是展开后的长文案。
    #[test]
    fn gate_blocked_still_expands_but_code_is_the_bare_field() {
        let text = r#"{"error":"GATE_BLOCKED","data":{"hard":[{"id":"g1","message":"缺验收","hint":"补场景"}]}}"#;
        let err = http_error(text);
        let ApiError::Http { detail, .. } = &err else {
            panic!("期望 Http 变体");
        };
        assert!(detail.contains("GATE_BLOCKED: g1 缺验收（补场景）"));
        assert_eq!(err.code(), Some("GATE_BLOCKED"));
    }

    #[test]
    fn non_http_variants_have_no_code_or_body() {
        let err = ApiError::Message("传输层失败".into());
        assert_eq!(err.code(), None);
        assert!(err.body().is_none());
    }
}
