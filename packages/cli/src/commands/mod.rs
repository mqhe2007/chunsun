
pub mod context;
pub mod defect;
pub mod env;


pub mod harness;
pub mod init;
pub mod repo;
pub mod requirement;
pub mod update;

use std::fmt;

#[derive(Debug)]
pub struct CmdError {
    message: String,
    exit_code: u8,
    silent: bool,
}

impl CmdError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 1,
            silent: false,
        }
    }

    pub fn with_code(message: impl Into<String>, code: u8) -> Self {
        Self {
            message: message.into(),
            exit_code: code,
            silent: false,
        }
    }

    pub fn exit_only(code: u8) -> Self {
        Self {
            message: String::new(),
            exit_code: code,
            silent: true,
        }
    }

    pub fn exit_code(&self) -> u8 {
        self.exit_code
    }

    pub fn is_silent(&self) -> bool {
        self.silent
    }
}

impl fmt::Display for CmdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CmdError {}

impl From<crate::api::ApiError> for CmdError {
    fn from(value: crate::api::ApiError) -> Self {
        CmdError::new(value.to_string())
    }
}

impl From<std::io::Error> for CmdError {
    fn from(value: std::io::Error) -> Self {
        CmdError::new(value.to_string())
    }
}

impl From<serde_json::Error> for CmdError {
    fn from(value: serde_json::Error) -> Self {
        CmdError::new(value.to_string())
    }
}

impl From<anyhow::Error> for CmdError {
    fn from(value: anyhow::Error) -> Self {
        CmdError::new(value.to_string())
    }
}

pub type CmdResult = Result<(), CmdError>;

pub fn print_json<T: serde::Serialize>(value: &T) -> CmdResult {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
