//! Bounded unary app requests carried by an admitted NIP-HOST operation.
use crate::{Code, Error, Result, Right};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CAPABILITY: &str = "app-rpc-v1";
/// Leave room for the host envelope within NIP-44's plaintext bound.
pub const MAX_BYTES: usize = 60 * 1024;

/// The app's unary dispatcher, called on a blocking worker after host admission.
/// Effects use the app's existing command ledger and the original command ID.
pub trait Handler: Send + Sync {
    fn call(
        &self,
        request: &str,
        device: &str,
        grant: Option<(&str, u64)>,
        app: &Request,
    ) -> std::result::Result<Answer, Code>;
}

/// Methods whose access requirements are known before app dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method {
    ListHarnesses,
    ListModels,
    ListSkills,
    ListCommands,
    RelayCommand,
}
impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ListHarnesses => "ListHarnesses",
            Self::ListModels => "ListModels",
            Self::ListSkills => "ListSkills",
            Self::ListCommands => "ListCommands",
            Self::RelayCommand => "RelayCommand",
        }
    }
    pub fn right(self) -> Right {
        match self {
            Self::RelayCommand => Right::Operate,
            _ => Right::Observe,
        }
    }
    pub fn reads_only(self) -> bool {
        self != Self::RelayCommand
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub method: Method,
    pub params: Value,
}
impl Request {
    pub fn validate(&self) -> Result<()> {
        bounded(self)?;
        if !self.params.is_object() {
            return Err(Error::new(
                Code::Malformed,
                "app parameters must be an object",
            ));
        }
        if self.method == Method::RelayCommand {
            for value in [&self.params["chatId"], &self.params["entry"]["id"]] {
                if !value.as_str().is_some_and(|s| {
                    !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
                }) {
                    return Err(Error::new(
                        Code::Malformed,
                        "app command needs its chat and command IDs",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    pub method: Method,
    pub value: Value,
}
impl Answer {
    pub fn validate(&self) -> Result<()> {
        bounded(self)?;
        if self.method == Method::RelayCommand
            && serde_json::to_vec(self).map_or(true, |bytes| bytes.len() > 4096)
        {
            return Err(Error::new(Code::Bounds, "app command reply is too large"));
        }
        Ok(())
    }
    pub fn answers(&self, request: &Request) -> bool {
        self.method == request.method
    }
}
fn bounded(value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| Error::new(Code::Malformed, "app request cannot be encoded"))?;
    if bytes.len() > MAX_BYTES {
        return Err(Error::new(
            Code::Bounds,
            "app request or reply is too large",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn methods_cannot_select_their_right_or_omit_command_identity() {
        assert!(serde_json::from_value::<Method>(json!("DeleteAll")).is_err());
        assert_eq!(Method::ListModels.right(), Right::Observe);
        assert_eq!(Method::RelayCommand.right(), Right::Operate);
        let mut request = Request {
            method: Method::RelayCommand,
            params: json!({"chatId":"chat"}),
        };
        assert!(request.validate().is_err());
        request.params["entry"] = json!({"id":"command"});
        request.validate().unwrap();
        request.params["padding"] = json!("x".repeat(MAX_BYTES));
        assert_eq!(request.validate().unwrap_err().code, Code::Bounds);
    }
}
