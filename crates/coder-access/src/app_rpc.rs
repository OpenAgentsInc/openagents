//! Bounded unary app requests carried by an admitted NIP-HOST operation.
use crate::{Code, Error, Result, Right};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CAPABILITY: &str = "app-rpc-v1";
/// Leave room for the host envelope within NIP-44's plaintext bound.
pub const MAX_BYTES: usize = 60 * 1024;

/// Current grant bounds, captured while the host holds its access lock.
pub struct Admission<'a> {
    pub rights: &'a crate::Rights,
    pub expires_at: u64,
}

/// The app's unary dispatcher, called on a blocking worker after host admission.
/// Effects use the app's existing command ledger and the original command ID.
pub trait Handler: Send + Sync {
    fn call_admitted(
        &self,
        request: &str,
        device: &str,
        grant: Option<(&str, u64)>,
        app: &Request,
        _admission: &Admission<'_>,
    ) -> std::result::Result<Answer, Code> {
        self.call(request, device, grant, app)
    }

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
    EngineInfo,
    AuthorizeSync,
    ListHarnesses,
    ListModels,
    ListSkills,
    ListCommands,
    RelayCommand,
    /// Run controls over this existing device channel.
    RunCommand,
    ContinueChat,
    PrepareContinuation,
    /// Sign this computer's request to join the phone's account (#12000):
    /// params `{url, challenge}`, answer `{event}`, a NIP-98 event signed by
    /// the computer's account key. The app answers it only while it shows
    /// a connect code that offers the link, and only to a pairing grant.
    LinkAccount,
    /// The phone's server accepted the link proof (#12000): params
    /// `{pubkey}`, the computer's account key; answer `{}`. The computer
    /// may sign in with that key now.
    AccountLinked,
}
impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EngineInfo => "EngineInfo",
            Self::AuthorizeSync => "AuthorizeSync",
            Self::ListHarnesses => "ListHarnesses",
            Self::ListModels => "ListModels",
            Self::ListSkills => "ListSkills",
            Self::ListCommands => "ListCommands",
            Self::RelayCommand => "RelayCommand",
            Self::RunCommand => "RunCommand",
            Self::ContinueChat => "ContinueChat",
            Self::PrepareContinuation => "PrepareContinuation",
            Self::LinkAccount => "LinkAccount",
            Self::AccountLinked => "AccountLinked",
        }
    }
    pub fn right(self) -> Right {
        match self {
            Self::RelayCommand
            | Self::RunCommand
            | Self::AuthorizeSync
            | Self::ContinueChat
            | Self::PrepareContinuation
            | Self::LinkAccount
            | Self::AccountLinked => Right::Operate,
            _ => Right::Observe,
        }
    }
    pub fn reads_only(self) -> bool {
        !matches!(
            self,
            Self::RelayCommand | Self::RunCommand | Self::ContinueChat | Self::PrepareContinuation
        )
    }
    pub fn first_missing(self, rights: &crate::Rights) -> Option<Right> {
        if !rights.contains(self.right()) {
            return Some(self.right());
        }
        (matches!(self, Self::ContinueChat | Self::PrepareContinuation)
            && !rights.contains(Right::Observe))
        .then_some(Right::Observe)
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
        if self.params.get("targetDeviceId").is_some() {
            return Err(Error::new(
                Code::Forbidden,
                "Choose this computer directly.",
            ));
        }
        if self.method == Method::RunCommand {
            for field in ["account", "run", "request_id"] {
                if !self.params[field].as_str().is_some_and(|s| {
                    !s.is_empty()
                        && s.len() <= 128
                        && s.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b))
                        && !matches!(s, "." | "..")
                }) {
                    return Err(Error::new(
                        Code::Malformed,
                        "Check the run command details.",
                    ));
                }
            }
            if !self.params["command"].is_object() {
                return Err(Error::new(Code::Malformed, "Choose a run command."));
            }
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
        if self.method == Method::ContinueChat {
            for value in [
                &self.params["chatId"],
                &self.params["account"],
                &self.params["target"]["id"],
                &self.params["target"]["home"],
            ] {
                if !value.as_str().is_some_and(|s| {
                    !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
                }) {
                    return Err(Error::new(
                        Code::Malformed,
                        "continuation needs its account, source, and destination",
                    ));
                }
            }
        }
        if self.method == Method::PrepareContinuation {
            for value in [
                &self.params["chatId"],
                &self.params["source"]["account"],
                &self.params["source"]["chat"],
            ] {
                if !value.as_str().is_some_and(|s| {
                    !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
                }) {
                    return Err(Error::new(
                        Code::Malformed,
                        "continuation needs its account, source, and destination",
                    ));
                }
            }
            if !self.params["source"]["revision"]
                .as_u64()
                .is_some_and(|r| r > 0)
            {
                return Err(Error::new(
                    Code::Malformed,
                    "continuation needs its revision",
                ));
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
        assert_eq!(Method::LinkAccount.right(), Right::Operate);
        assert!(Method::LinkAccount.reads_only());
        assert_eq!(Method::AccountLinked.right(), Right::Operate);
        assert!(Method::AccountLinked.reads_only());
        let mut request = Request {
            method: Method::RelayCommand,
            params: json!({"chatId":"chat"}),
        };
        assert!(request.validate().is_err());
        request.params["entry"] = json!({"id":"command"});
        request.validate().unwrap();
        request.params["targetDeviceId"] = json!("another-computer");
        assert_eq!(request.validate().unwrap_err().code, Code::Forbidden);
        request
            .params
            .as_object_mut()
            .unwrap()
            .remove("targetDeviceId");
        request.params["padding"] = json!("x".repeat(MAX_BYTES));
        assert_eq!(request.validate().unwrap_err().code, Code::Bounds);
    }
}
