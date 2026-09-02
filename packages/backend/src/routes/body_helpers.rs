//! 共享的请求体小类型（被多个域路由复用）。

use serde::Deserialize;

use crate::core::serde_ext::double_option;

/// `BlockedBy` 引用（创建需求/缺陷时携带的"被谁阻塞"上游节点）。
///
/// 字段均为三态 `Option<Option<String>>`：缺省 = 不传，显式 `null` 422（与仓库内其他
/// 可空字段策略一致）。`kind` 必须是 `requirement` / `defect` 之一（非法值由 service
/// 层 → `DEPENDENCY_TARGET_NOT_FOUND`）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedByRefBody {
    #[serde(default, deserialize_with = "double_option")]
    pub kind: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub id: Option<Option<String>>,
}
