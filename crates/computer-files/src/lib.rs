//! Shared bounds and results for workspace file transfers.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_PATH_BYTES: usize = 1024;
pub const TIMEOUT_SECONDS: u64 = 60;
pub const MAX_TRANSFERS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileError {
    Invalid,
    Missing,
    Escape,
    Special,
    TooLarge,
    Changed,
    Exists,
    Incomplete,
    Unsupported,
    Unavailable,
    WriteUncertain,
}

impl FileError {
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Invalid => 10,
            Self::Missing => 11,
            Self::Escape => 12,
            Self::Special => 13,
            Self::TooLarge => 14,
            Self::Changed => 15,
            Self::Exists => 16,
            Self::Incomplete => 17,
            Self::Unsupported => 18,
            Self::Unavailable => 19,
            Self::WriteUncertain => 20,
        }
    }
    pub fn from_exit(code: i32) -> Self {
        match code {
            10 => Self::Invalid,
            11 => Self::Missing,
            12 => Self::Escape,
            13 => Self::Special,
            14 => Self::TooLarge,
            15 => Self::Changed,
            16 => Self::Exists,
            17 => Self::Incomplete,
            18 => Self::Unsupported,
            20 => Self::WriteUncertain,
            _ => Self::Unavailable,
        }
    }
    pub fn message(self) -> &'static str {
        match self {
            Self::Invalid => {
                "Choose a relative file path without empty components, percent signs, or parent directories."
            }
            Self::Missing => "The file or its parent directory does not exist.",
            Self::Escape => "File paths cannot follow links or leave the workspace.",
            Self::Special => "Choose a regular file.",
            Self::TooLarge => "The file exceeds this computer's transfer limit.",
            Self::Changed => "The file changed during this transfer. Try reading it again.",
            Self::Exists => "A different file already exists at that path.",
            Self::Incomplete => "The upload length or checksum does not match.",
            Self::Unsupported => "File transfers are unavailable on this computer.",
            Self::Unavailable => "The file transfer could not finish. Try again shortly.",
            Self::WriteUncertain => {
                "The upload result is unavailable. Read the file to check its checksum before retrying."
            }
        }
    }
}

pub fn validate_path(path: &str) -> Result<(), FileError> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path
            .chars()
            .any(|c| c.is_control() || matches!(c, '\\' | '%' | ':' | '\u{fffd}'))
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == ".." || p.len() > 255)
    {
        return Err(FileError::Invalid);
    }
    Ok(())
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileInfo {
    pub size: usize,
    pub sha256: String,
}
