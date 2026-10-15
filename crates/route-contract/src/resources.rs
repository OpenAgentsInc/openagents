//! Owner-bound resource metadata. References contain no filesystem paths.

use crate::{Digest, digest_of, observation::Scope};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "openagents.coder.resources.v1";
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_INPUTS: usize = 4;
pub const MAX_TASK_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_DIMENSION: u32 = 4096;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owner {
    pub host: String,
    pub task: String,
}
impl Owner {
    pub fn valid(&self) -> bool {
        bounded(&self.host, 256) && bounded(&self.task, 128)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Origin {
    Initial,
    Input { message: String },
    Produced { scope: Scope, item: String },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    Png,
    Jpeg,
    Text,
    Json,
}
impl MediaType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Text => "text/plain",
            Self::Json => "application/json",
        }
    }
    pub fn image(self) -> bool {
        matches!(self, Self::Png | Self::Jpeg)
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "image/png" => Some(Self::Png),
            "image/jpeg" => Some(Self::Jpeg),
            "text/plain" => Some(Self::Text),
            "application/json" => Some(Self::Json),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Usage {
    PromptImage,
    Preview,
    Read,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}
impl Dimensions {
    pub fn valid(self) -> bool {
        self.width > 0
            && self.height > 0
            && self.width <= MAX_DIMENSION
            && self.height <= MAX_DIMENSION
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub schema: String,
    pub id: Digest,
    pub owner: Owner,
    pub origin: Origin,
    pub digest: Digest,
    pub media_type: MediaType,
    pub size: usize,
    pub name: String,
    pub dimensions: Option<Dimensions>,
    pub uses: Vec<Usage>,
}
impl Reference {
    pub fn seal(&mut self) {
        self.id = self.identity();
    }
    fn identity(&self) -> Digest {
        digest_of(&(
            &self.schema,
            &self.owner,
            &self.origin,
            &self.digest,
            self.media_type,
            self.size,
            &self.name,
            self.dimensions,
            &self.uses,
        ))
    }
    pub fn valid(&self) -> bool {
        self.schema == SCHEMA
            && self.owner.valid()
            && self.id == self.identity()
            && self.size > 0
            && self.size <= MAX_BYTES
            && self.name.len() <= 96
            && !self.name.chars().any(char::is_control)
            && self
                .digest
                .as_str()
                .strip_prefix("sha256:")
                .is_some_and(|hex| {
                    hex.len() == 64
                        && hex
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
            && if self.media_type.image() {
                self.dimensions.is_some_and(Dimensions::valid)
            } else {
                self.dimensions.is_none()
            }
            && !self.uses.is_empty()
            && self.uses.len() <= 3
            && self.uses.iter().enumerate().all(|(index, usage)| {
                !self.uses[..index].contains(usage)
                    && (*usage != Usage::PromptImage
                        || (self.media_type.image()
                            && matches!(self.origin, Origin::Input { .. } | Origin::Initial)))
            })
            && match &self.origin {
                Origin::Initial => true,
                Origin::Input { message } => bounded(message, 128),
                Origin::Produced { scope, item } => {
                    scope.host == self.owner.host
                        && scope.task == self.owner.task
                        && scope.turn > 0
                        && scope.generation > 0
                        && bounded(item, 256)
                }
            }
    }
    pub fn allows(&self, usage: Usage) -> bool {
        self.valid() && self.uses.contains(&usage)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Input {
    pub schema: String,
    pub id: Digest,
    pub message: String,
    pub turn: usize,
    pub owner: Owner,
    pub text: Digest,
    pub choice: Option<Digest>,
    pub resources: Vec<Reference>,
}
impl Input {
    pub fn seal(&mut self) {
        self.id = digest_of(&(
            &self.schema,
            &self.message,
            self.turn,
            &self.owner,
            &self.text,
            &self.choice,
            &self.resources,
        ));
    }
    pub fn valid_for(&self, task: &str, text: &str) -> bool {
        if self.resources.is_empty()
            || self.resources.len() > MAX_INPUTS
            || text.len() > 32 * 1024
            || text.trim().is_empty()
            || !(1..=64).contains(&self.turn)
            || !self.owner.valid()
            || !bounded(&self.message, 128)
        {
            return false;
        }
        let mut sealed = self.clone();
        sealed.seal();
        self.schema == SCHEMA
            && self.id == sealed.id
            && self.owner.valid()
            && self.owner.task == task
            && bounded(&self.message, 128)
            && self.text == Digest::of_bytes(text.as_bytes())
            && !self.resources.is_empty()
            && self.resources.len() <= MAX_INPUTS
            && self.resources.iter().all(|resource| {
                resource.allows(Usage::PromptImage)
                    && resource.owner == self.owner
                    && resource.origin
                        == Origin::Input {
                            message: self.message.clone(),
                        }
            })
            && self
                .resources
                .iter()
                .map(|resource| resource.size)
                .sum::<usize>()
                <= MAX_INPUT_BYTES
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub schema: String,
    pub input: Digest,
    pub task: String,
    pub turn: usize,
    pub revision: u64,
}
impl Receipt {
    pub fn valid_for(&self, input: &Input) -> bool {
        self.schema == SCHEMA
            && self.input == input.id
            && self.task == input.owner.task
            && self.turn == input.turn
            && self.revision > 0
    }
}
fn bounded(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

/// A display name bounded in bytes, including names containing Unicode.
pub fn name(value: &str) -> String {
    let mut value: String = value.chars().filter(|c| !c.is_control()).collect();
    while value.len() > 96 {
        value.pop();
    }
    if value.is_empty() {
        "Attachment".into()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image() -> Reference {
        let bytes = include_bytes!("../../coder-sdk/fixtures/resources/one-pixel.png");
        let mut reference = Reference {
            schema: SCHEMA.into(),
            id: digest_of(&()),
            owner: Owner {
                host: "local".into(),
                task: "task".into(),
            },
            origin: Origin::Input {
                message: "message".into(),
            },
            digest: Digest::of_bytes(bytes),
            media_type: MediaType::Png,
            size: bytes.len(),
            name: "one.png".into(),
            dimensions: Some(Dimensions {
                width: 1,
                height: 1,
            }),
            uses: vec![Usage::PromptImage, Usage::Preview],
        };
        reference.seal();
        reference
    }
    #[test]
    fn references_bind_ownership_type_size_and_use_and_inputs_reject_overflow() {
        let reference = image();
        assert!(reference.valid());
        let mut changed = reference.clone();
        changed.owner.task = "other".into();
        assert!(!changed.valid());
        changed.seal();
        assert!(changed.valid());
        changed.origin = Origin::Produced {
            scope: Scope {
                host: "local".into(),
                task: "other".into(),
                turn: 1,
                generation: 1,
            },
            item: "result".into(),
        };
        changed.seal();
        assert!(!changed.valid());
        let mut input = Input {
            schema: SCHEMA.into(),
            id: digest_of(&()),
            message: "message".into(),
            turn: 2,
            owner: reference.owner.clone(),
            text: Digest::of_bytes(b"inspect"),
            choice: None,
            resources: vec![reference],
        };
        input.seal();
        assert!(input.valid_for("task", "inspect"));
        assert!(!input.valid_for("other", "inspect"));
        assert!(!input.valid_for("task", "changed"));
        input.resources[0].size = usize::MAX;
        input.seal();
        assert!(!input.valid_for("task", "inspect"));
        assert!(name(&"名".repeat(100)).len() <= 96);
    }
    #[test]
    fn absent_image_support_keeps_old_model_offer_bytes() {
        use crate::session::control::Model;
        let raw = serde_json::json!({"selection":{"provider":"claude","model":"fixture","effort":null},"adapter":"claude-sdk","efforts":[],"available":true,"modes":[],"account":null,"reason":null});
        let model: Model = serde_json::from_value(raw.clone()).unwrap();
        assert!(!model.image_followups);
        assert_eq!(serde_json::to_value(model).unwrap(), raw);
    }
}
