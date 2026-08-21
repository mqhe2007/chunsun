//! 全局中间件：安全响应头 + 内存限流（对齐旧后端 securityHeaders.ts / rateLimit.ts）。

pub mod rate_limit;
pub mod require_ready;
pub mod security_headers;
