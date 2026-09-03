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
            return Err(ApiError::Message(format!(
                "{method} {path} 失败 ({status}): {detail}"
            )));
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
