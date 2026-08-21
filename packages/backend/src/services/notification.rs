//! 通知服务（对齐 `notificationService.ts`）。

use sqlx::PgPool;

use crate::api::AppError;
use crate::repos::notification::{create_notification, NotificationInput};

pub async fn notify_user(
    pool: &PgPool,
    data: NotificationData,
) -> Result<(), AppError> {
    create_notification(
        pool,
        NotificationInput {
            user_id: data.user_id,
            ty: data.ty,
            title: data.title,
            body: data.body,
            link: data.link,
        },
    )
    .await
}

#[derive(Debug, Clone)]
pub struct NotificationData {
    pub user_id: String,
    pub ty: String,
    pub title: String,
    pub body: Option<String>,
    pub link: Option<String>,
}
