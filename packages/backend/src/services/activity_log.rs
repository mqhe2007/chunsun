//! 项目活动日志（1:1 移植自 `services/activityLogService.ts`）。
//!
//! 动作→中文描述的映射表是**用户可见文案**，必须逐字对齐，否则活动流页面会出现文案漂移。

use sqlx::PgPool;

use crate::api::AppError;
use crate::repos::project_activity::{create_project_activity, CreateProjectActivityInput};

/// 对齐 TS 的 `ActivityAction` 联合类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ActivityAction {
    ProjectCreated,
    ProjectUpdated,
    ProjectDeleted,
    ProjectInitialized,
    RequirementCreated,
    RequirementUpdated,
    FeatureCreated,
    FeatureStage,
    FeatureDocument,
    FeatureSchedule,
    DefectCreated,
    DefectUpdated,
    DefectDeleted,
    DefectConverted,
    EnvVarCreated,
    EnvVarUpdated,
    EnvVarDeleted,
    ApplicationCreated,
    ApplicationUpdated,
    ApplicationDeleted,
    ModuleCreated,
    ModuleUpdated,
    ModuleDeleted,
}

impl ActivityAction {
    /// 写入 `action` 列的字面量。
    pub fn as_str(&self) -> &'static str {
        match self {
            ActivityAction::ProjectCreated => "PROJECT_CREATED",
            ActivityAction::ProjectUpdated => "PROJECT_UPDATED",
            ActivityAction::ProjectDeleted => "PROJECT_DELETED",
            ActivityAction::ProjectInitialized => "PROJECT_INITIALIZED",
            ActivityAction::RequirementCreated => "REQUIREMENT_CREATED",
            ActivityAction::RequirementUpdated => "REQUIREMENT_UPDATED",
            ActivityAction::FeatureCreated => "FEATURE_CREATED",
            ActivityAction::FeatureStage => "FEATURE_STAGE",
            ActivityAction::FeatureDocument => "FEATURE_DOCUMENT",
            ActivityAction::FeatureSchedule => "FEATURE_SCHEDULE",
            ActivityAction::DefectCreated => "DEFECT_CREATED",
            ActivityAction::DefectUpdated => "DEFECT_UPDATED",
            ActivityAction::DefectDeleted => "DEFECT_DELETED",
            ActivityAction::DefectConverted => "DEFECT_CONVERTED",
            ActivityAction::EnvVarCreated => "ENV_VAR_CREATED",
            ActivityAction::EnvVarUpdated => "ENV_VAR_UPDATED",
            ActivityAction::EnvVarDeleted => "ENV_VAR_DELETED",
            ActivityAction::ApplicationCreated => "APPLICATION_CREATED",
            ActivityAction::ApplicationUpdated => "APPLICATION_UPDATED",
            ActivityAction::ApplicationDeleted => "APPLICATION_DELETED",
            ActivityAction::ModuleCreated => "MODULE_CREATED",
            ActivityAction::ModuleUpdated => "MODULE_UPDATED",
            ActivityAction::ModuleDeleted => "MODULE_DELETED",
        }
    }

    /// `activityDescriptions` 默认文案。
    pub fn default_description(&self) -> &'static str {
        match self {
            ActivityAction::ProjectCreated => "创建项目",
            ActivityAction::ProjectUpdated => "更新项目信息",
            ActivityAction::ProjectDeleted => "删除项目",
            ActivityAction::ProjectInitialized => "初始化项目完成",
            ActivityAction::RequirementCreated => "创建需求",
            ActivityAction::RequirementUpdated => "更新需求",
            ActivityAction::FeatureCreated => "创建特性",
            ActivityAction::FeatureStage => "推进特性阶段",
            ActivityAction::FeatureDocument => "更新特性产物",
            ActivityAction::FeatureSchedule => "更新特性排期",
            ActivityAction::DefectCreated => "创建缺陷",
            ActivityAction::DefectUpdated => "更新缺陷",
            ActivityAction::DefectDeleted => "删除缺陷",
            ActivityAction::DefectConverted => "缺陷转需求",
            ActivityAction::EnvVarCreated => "创建环境变量",
            ActivityAction::EnvVarUpdated => "更新环境变量",
            ActivityAction::EnvVarDeleted => "删除环境变量",
            ActivityAction::ApplicationCreated => "创建应用",
            ActivityAction::ApplicationUpdated => "更新应用",
            ActivityAction::ApplicationDeleted => "删除应用",
            ActivityAction::ModuleCreated => "创建模块",
            ActivityAction::ModuleUpdated => "更新模块",
            ActivityAction::ModuleDeleted => "删除模块",
        }
    }
}

/// 可选项，对齐 TS 的 `options?` 参数。
#[derive(Debug, Default, Clone)]
pub struct LogActivityOptions<'a> {
    pub entity_type: Option<&'a str>,
    pub entity_id: Option<&'a str>,
    pub metadata: Option<serde_json::Value>,
    pub description: Option<&'a str>,
}

pub async fn log_activity(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    action: ActivityAction,
    options: LogActivityOptions<'_>,
) -> Result<(), AppError> {
    let description = options.description.unwrap_or_else(|| action.default_description());
    create_project_activity(
        pool,
        CreateProjectActivityInput {
            project_id,
            user_id,
            action: action.as_str(),
            description,
            entity_type: options.entity_type,
            entity_id: options.entity_id,
            metadata: options.metadata,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_literals_match_legacy() {
        assert_eq!(ActivityAction::ProjectCreated.as_str(), "PROJECT_CREATED");
        assert_eq!(ActivityAction::ProjectUpdated.as_str(), "PROJECT_UPDATED");
        assert_eq!(ActivityAction::DefectConverted.as_str(), "DEFECT_CONVERTED");
    }

    #[test]
    fn descriptions_match_legacy_copy() {
        assert_eq!(ActivityAction::ProjectCreated.default_description(), "创建项目");
        assert_eq!(
            ActivityAction::ProjectUpdated.default_description(),
            "更新项目信息"
        );
        assert_eq!(
            ActivityAction::ProjectInitialized.default_description(),
            "初始化项目完成"
        );
        assert_eq!(ActivityAction::FeatureStage.default_description(), "推进特性阶段");
    }

    #[test]
    fn explicit_description_wins() {
        let opts = LogActivityOptions {
            description: Some("自定义"),
            ..Default::default()
        };
        let resolved = opts
            .description
            .unwrap_or_else(|| ActivityAction::ProjectCreated.default_description());
        assert_eq!(resolved, "自定义");
    }
}
