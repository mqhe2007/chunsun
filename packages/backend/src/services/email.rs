//! 邮件服务（对齐 `emailService.ts`）：SMTP 未配置时不抛错，记 EmailLog 失败后继续。
//! 测试发信走 `try_send_email`，把成败回传给调用方。

use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use sqlx::PgPool;

use crate::api::AppError;
use crate::repos::email_log::{create_email_log, EmailLogInput};
use crate::services::settings::{get_smtp_config, SmtpConfig};

pub struct EmailOptions {
    pub to: String,
    pub subject: String,
    pub html: String,
    pub template: String,
}

fn make_from(config: &SmtpConfig) -> Result<Mailbox, String> {
    format!("\"{}\" <{}>", config.from_name, config.from_address)
        .parse::<Mailbox>()
        .map_err(|e| format!("发件地址无效: {e}"))
}

/// 尝试发送邮件：成功返回 Ok，失败返回可读错误（同时写入 EmailLog）。
pub async fn try_send_email(pool: &PgPool, options: EmailOptions) -> Result<(), String> {
    let config = match get_smtp_config(pool).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = ?e, "smtp config load failed");
            return Err("读取 SMTP 配置失败".into());
        }
    };

    if config.host.is_empty() || config.user.is_empty() || config.from_address.is_empty() {
        let error = "SMTP not configured".to_string();
        tracing::warn!(to = %options.to, template = %options.template, "{error}");
        let _ = create_email_log(
            pool,
            EmailLogInput {
                to: options.to,
                subject: options.subject,
                template: options.template,
                status: "failed".to_string(),
                error: Some(error.clone()),
            },
        )
        .await;
        return Err("SMTP 未配置完整：请填写发件人邮箱、SMTP 服务器与用户名并保存".into());
    }

    let from = match make_from(&config) {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(error = %e, "invalid from address");
            let _ = create_email_log(
                pool,
                EmailLogInput {
                    to: options.to,
                    subject: options.subject,
                    template: options.template,
                    status: "failed".to_string(),
                    error: Some(e.clone()),
                },
            )
            .await;
            return Err(e);
        }
    };
    let to = match options.to.parse::<Mailbox>() {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, to = %options.to, "invalid to address");
            let error = "收件人邮箱格式无效".to_string();
            let _ = create_email_log(
                pool,
                EmailLogInput {
                    to: options.to,
                    subject: options.subject,
                    template: options.template,
                    status: "failed".to_string(),
                    error: Some("invalid recipient".to_string()),
                },
            )
            .await;
            return Err(error);
        }
    };

    let mut builder = Message::builder().from(from).to(to).subject(&options.subject);
    builder = builder.header(lettre::message::header::ContentType::TEXT_HTML);
    let email = match builder.body(options.html.clone()) {
        Ok(e) => e,
        Err(e) => {
            let error = format!("邮件构造失败: {e}");
            tracing::error!(error = %error);
            let _ = create_email_log(
                pool,
                EmailLogInput {
                    to: options.to,
                    subject: options.subject,
                    template: options.template,
                    status: "failed".to_string(),
                    error: Some(error.clone()),
                },
            )
            .await;
            return Err(error);
        }
    };

    let mailer = if config.secure {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host).map(|t| t.port(config.port))
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
            .map(|t| t.port(config.port))
    };

    let mailer = match mailer {
        Ok(m) => {
            if !config.user.is_empty() {
                m.credentials(Credentials::new(config.user.clone(), config.password.clone()))
            } else {
                m
            }
            .build()
        }
        Err(e) => {
            let error = format!("SMTP 连接配置失败: {e}");
            tracing::error!(host = %config.host, "{error}");
            let _ = create_email_log(
                pool,
                EmailLogInput {
                    to: options.to,
                    subject: options.subject,
                    template: options.template,
                    status: "failed".to_string(),
                    error: Some(error.clone()),
                },
            )
            .await;
            return Err(error);
        }
    };

    match mailer.send(email).await {
        Ok(_) => {
            let _ = create_email_log(
                pool,
                EmailLogInput {
                    to: options.to,
                    subject: options.subject,
                    template: options.template,
                    status: "sent".to_string(),
                    error: None,
                },
            )
            .await;
            Ok(())
        }
        Err(e) => {
            let error = e.to_string();
            tracing::error!(to = %options.to, template = %options.template, error = %error, "failed to send email");
            let _ = create_email_log(
                pool,
                EmailLogInput {
                    to: options.to,
                    subject: options.subject,
                    template: options.template,
                    status: "failed".to_string(),
                    error: Some(error.clone()),
                },
            )
            .await;
            Err(format!("发送失败: {error}"))
        }
    }
}

/// 业务邮件：失败只记日志，不向上抛（对齐旧端静默策略）。
pub async fn send_email(pool: &PgPool, options: EmailOptions) {
    if let Err(e) = try_send_email(pool, options).await {
        tracing::debug!(error = %e, "send_email swallowed failure");
    }
}

/// 管理员测试发信：把成败回传给 API。
pub async fn send_test_email(pool: &PgPool, to: &str) -> Result<(), AppError> {
    let to = to.trim();
    if to.is_empty() {
        return Err(AppError::bad_request("INVALID_EMAIL").with_message("请填写收件人邮箱"));
    }

    try_send_email(
        pool,
        EmailOptions {
            to: to.to_string(),
            template: "test".to_string(),
            subject: "春笋平台 · 测试邮件".to_string(),
            html: "<p>你好，</p><p>这是一封来自春笋平台的测试邮件。若你收到此信，说明当前 SMTP 配置可用。</p>".to_string(),
        },
    )
    .await
    .map_err(|e| AppError::bad_request("EMAIL_SEND_FAILED").with_message(e))
}

pub async fn send_verification_email(pool: &PgPool, to: &str, token: &str, origin: &str) {
    let verify_url = format!("{origin}/console/auth/verify-email?token={token}");
    send_email(
        pool,
        EmailOptions {
            to: to.to_string(),
            template: "verification".to_string(),
            subject: "验证你的春笋账户邮箱".to_string(),
            html: format!(
                "<p>你好，</p><p>感谢你注册春笋。请点击下方链接验证邮箱：</p><p><a href=\"{verify_url}\" target=\"_blank\">{verify_url}</a></p><p>链接 24 小时内有效。如非本人操作，请忽略此邮件。</p>"
            ),
        },
    )
    .await;
}

pub async fn send_password_reset_email(pool: &PgPool, to: &str, token: &str, origin: &str) {
    let reset_url = format!("{origin}/console/auth/reset-password?token={token}");
    send_email(
        pool,
        EmailOptions {
            to: to.to_string(),
            template: "password-reset".to_string(),
            subject: "重置你的春笋账户密码".to_string(),
            html: format!(
                "<p>你好，</p><p>你申请了重置密码。请点击下方链接：</p><p><a href=\"{reset_url}\" target=\"_blank\">{reset_url}</a></p><p>链接 1 小时内有效。如非本人操作，请忽略此邮件。</p>"
            ),
        },
    )
    .await;
}

/// 项目邀请等站内通知邮件：对齐旧后端 `sendNotificationEmail(...).catch(()=>{})`，
/// 邮件发送失败静默吞掉，不影响主流程（与 auth 域 `.await?` 传播语义相反）。
pub async fn send_notification_email(pool: &PgPool, to: &str, title: &str, body: &str, link: &str) {
    send_email(
        pool,
        EmailOptions {
            to: to.to_string(),
            template: "notification".to_string(),
            subject: title.to_string(),
            html: format!(
                "<p>{body}</p><p><a href=\"{link}\" target=\"_blank\">{link}</a></p>"
            ),
        },
    )
    .await;
}
