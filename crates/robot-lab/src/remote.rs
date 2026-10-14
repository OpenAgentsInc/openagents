//! Bounded Robot Lab reads and explicit controls carried by a paired host.
use crate::Reference;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CHUNK_BYTES: usize = 16 * 1024;
pub const MAX_REPLY_BYTES: usize = 48 * 1024;
pub const MAX_VIEW_BYTES: usize = 18 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    List {
        offset: u32,
    },
    View {
        reference: Reference,
        offset: u64,
    },
    Asset {
        sha256: String,
        offset: u64,
    },
    Preview {
        reference: Reference,
        frame: u32,
    },
    Run {
        experiment: Reference,
        seed: u64,
        job: String,
        batch: bool,
    },
    Status {
        job: String,
    },
    Control {
        job: String,
        command: Control,
    },
    Pose {
        reference: Reference,
        joint: String,
        position: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Control {
    Pause,
    Resume,
    Step,
    Stop,
    Reset { next_job: String, seed: u64 },
}
impl Request {
    pub fn reads_only(&self) -> bool {
        matches!(
            self,
            Self::List { .. }
                | Self::View { .. }
                | Self::Asset { .. }
                | Self::Preview { .. }
                | Self::Status { .. }
        )
    }
    pub fn validate(&self) -> Result<(), String> {
        let name = |s: &str| {
            !s.is_empty()
                && s.len() <= 128
                && s.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
        };
        match self {
            Self::List { offset } => {
                if *offset > 10000 || *offset % 16 != 0 {
                    return Err("Choose a robot lab page".into());
                }
            }
            Self::View { reference, offset } => {
                reference.validate().map_err(|e| e.to_string())?;
                check_offset(*offset)?;
            }
            Self::Asset { sha256, offset } => {
                if sha256.len() != 64
                    || !sha256
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                {
                    return Err("Choose a saved robot file".into());
                }
                check_offset(*offset)?;
            }
            Self::Preview { reference, .. } => reference.validate().map_err(|e| e.to_string())?,
            Self::Run {
                experiment, job, ..
            } => {
                experiment.validate().map_err(|e| e.to_string())?;
                if !name(job) {
                    return Err("Choose a short run name".into());
                }
            }
            Self::Status { job } | Self::Control { job, .. } => {
                if !name(job) {
                    return Err("Choose a saved run".into());
                }
                if let Self::Control {
                    command: Control::Reset { next_job, .. },
                    ..
                } = self
                {
                    if !name(next_job) {
                        return Err("Choose a new run name".into());
                    }
                }
            }
            Self::Pose {
                reference,
                joint,
                position,
            } => {
                reference.validate().map_err(|e| e.to_string())?;
                if joint.is_empty()
                    || joint.len() > 128
                    || joint.chars().any(char::is_control)
                    || position.len() > 32
                    || !position.parse::<f64>().is_ok_and(f64::is_finite)
                {
                    return Err("Enter a joint position".into());
                }
            }
        }
        Ok(())
    }
}
fn check_offset(offset: u64) -> Result<(), String> {
    if offset > 256 << 20 || offset % CHUNK_BYTES as u64 != 0 {
        return Err("This robot file position couldn't be read".into());
    }
    Ok(())
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Answer {
    Data {
        request: Request,
        value: Value,
    },
    Chunk {
        request: Request,
        sha256: String,
        size: u64,
        offset: u64,
        bytes: String,
    },
}
impl Answer {
    pub fn validate(&self) -> Result<(), String> {
        let request = match self {
            Self::Data { request, .. } | Self::Chunk { request, .. } => request,
        };
        request.validate()?;
        if serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > MAX_REPLY_BYTES {
            return Err("This robot reply is too large".into());
        }
        match self {
            Self::Data { request, .. }
                if matches!(request, Request::View { .. } | Request::Asset { .. }) =>
            {
                return Err("This robot file reply couldn't be read".into());
            }
            Self::Chunk {
                request,
                sha256,
                size,
                offset,
                bytes,
            } => {
                let expected = match request {
                    Request::View { offset, .. } | Request::Asset { offset, .. } => *offset,
                    _ => return Err("This robot file reply couldn't be read".into()),
                };
                let max = if matches!(request, Request::View { .. }) {
                    MAX_VIEW_BYTES as u64
                } else {
                    256 << 20
                };
                if sha256.len() != 64
                    || !sha256
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                    || *size == 0
                    || *size > max
                    || *offset != expected
                    || *offset >= *size
                    || decode(bytes)?.len() != CHUNK_BYTES.min((*size - *offset) as usize)
                {
                    return Err("This robot file reply is incomplete".into());
                }
                if let Request::Asset {
                    sha256: expected, ..
                } = request
                {
                    if sha256 != expected {
                        return Err("This robot file changed".into());
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }
    pub fn answers(&self, request: &Request) -> bool {
        match self {
            Self::Data {
                request: actual, ..
            }
            | Self::Chunk {
                request: actual, ..
            } => actual == request,
        }
    }
}

pub fn decode(bytes: &str) -> Result<Vec<u8>, String> {
    if bytes.len() > (CHUNK_BYTES.div_ceil(3) * 4) {
        return Err("This robot file chunk is too large".into());
    }
    base64::engine::general_purpose::STANDARD
        .decode(bytes)
        .map_err(|_| "This robot file chunk couldn't be read".into())
}
pub fn chunk(request: Request, content: &[u8], offset: u64) -> Result<Answer, String> {
    use sha2::{Digest, Sha256};
    if offset >= content.len() as u64 {
        return Err("This robot file position couldn't be read".into());
    }
    let answer = Answer::Chunk {
        request,
        sha256: format!("{:x}", Sha256::digest(content)),
        size: content.len() as u64,
        offset,
        bytes: base64::engine::general_purpose::STANDARD
            .encode(&content[offset as usize..content.len().min(offset as usize + CHUNK_BYTES)]),
    };
    answer.validate()?;
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunks_check_alignment_size_identity_and_wire_bounds() {
        let content = vec![255; CHUNK_BYTES * 2 + 3];
        let reference = Reference {
            document: "recorded".into(),
            version: "a".repeat(64),
        };
        let request = Request::View {
            reference,
            offset: 0,
        };
        let answer = chunk(request.clone(), &content, 0).unwrap();
        assert!(answer.answers(&request));
        assert!(serde_json::to_vec(&answer).unwrap().len() < MAX_REPLY_BYTES);
        if let Answer::Chunk { bytes, .. } = &answer {
            assert_eq!(decode(bytes).unwrap(), &content[..CHUNK_BYTES]);
        }
        let mut altered = answer.clone();
        if let Answer::Chunk { offset, .. } = &mut altered {
            *offset = CHUNK_BYTES as u64;
        }
        assert!(altered.validate().is_err());
        let mut altered = answer.clone();
        if let Answer::Chunk { size, .. } = &mut altered {
            *size = MAX_VIEW_BYTES as u64 + 1;
        }
        assert!(altered.validate().is_err());
        let mut altered = answer;
        if let Answer::Chunk { bytes, .. } = &mut altered {
            bytes.push('!');
        }
        assert!(altered.validate().is_err());
        assert!(
            Request::Asset {
                sha256: "../private".into(),
                offset: 0
            }
            .validate()
            .is_err()
        );
        assert!(
            Request::Asset {
                sha256: "a".repeat(64),
                offset: 1
            }
            .validate()
            .is_err()
        );
        assert!(
            Request::Control {
                job: "../run".into(),
                command: Control::Stop
            }
            .validate()
            .is_err()
        );
    }
}
