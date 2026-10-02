//! Bringing the owner's wallet to a computer (`openagents wallet link`).
//!
//! A command on the computer records an [`Ask`] in its host's book: a
//! one-time public key and the computer's name. The phone reads the open
//! asks with `wallet.link.list`, shows the code derived from the key beside
//! the computer's name, and, when the owner approves, answers with
//! `wallet.link.answer` carrying the wallet seed [`Sealed`] to that key with
//! NIP-44. Declining answers with no envelope. The host only relays the
//! envelope; it never holds the seed in the clear.

use crate::{Code, Error, Result, fail};
use serde::{Deserialize, Serialize};

/// The most open asks one `wallet.link.list` answer carries.
pub const MAX_LISTED: usize = 8;
/// The longest computer name, in bytes.
pub const MAX_COMPUTER: usize = 120;
/// The longest NIP-44 payload an envelope carries, in characters.
pub const MAX_PAYLOAD: usize = 4096;

/// One computer's request for the wallet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ask {
    /// 32 lowercase hex characters.
    pub id: String,
    /// The computer's one-time x-only public key, 64 lowercase hex.
    pub key: String,
    /// The computer's name, as its owner sees it.
    pub computer: String,
    pub created_at: u64,
    pub expires_at: u64,
}

impl Ask {
    /// # Errors
    /// A malformed field.
    pub fn validate(&self) -> Result<()> {
        id(&self.id)?;
        key(&self.key)?;
        if self.computer.is_empty()
            || self.computer.len() > MAX_COMPUTER
            || self.computer.chars().any(char::is_control)
        {
            return fail(Code::Bounds, "the computer name exceeds its bound");
        }
        if self.expires_at <= self.created_at {
            return fail(
                Code::Malformed,
                "a wallet link ask must end after it starts",
            );
        }
        Ok(())
    }
}

/// The wallet seed sealed to an ask's key: the sender's one-time key and
/// the NIP-44 v2 payload (`openagents_spark::link::Sealed` has this shape).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sealed {
    pub v: String,
    pub from: String,
    pub payload: String,
}

impl Sealed {
    /// # Errors
    /// A malformed field.
    pub fn validate(&self) -> Result<()> {
        if self.v.is_empty() || self.v.len() > 64 || self.v.chars().any(char::is_control) {
            return fail(Code::Malformed, "unknown wallet link envelope");
        }
        key(&self.from)?;
        if self.payload.len() > MAX_PAYLOAD {
            return fail(Code::Bounds, "the wallet link envelope is too large");
        }
        nostr::nip44::payload_shape(&self.payload)
            .map_err(|_| Error::new(Code::Malformed, "the wallet link envelope is not NIP-44"))
    }
}

/// An ask's ID: 32 lowercase hex characters.
///
/// # Errors
/// Anything else.
pub fn id(value: &str) -> Result<()> {
    if value.len() == 32
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        Ok(())
    } else {
        fail(Code::Malformed, "a wallet link ID is 32 lowercase hex")
    }
}

fn key(value: &str) -> Result<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        Ok(())
    } else {
        fail(Code::Malformed, "a wallet link key is 64 lowercase hex")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Operation, Outcome};
    use crate::rights::Right;

    const KEY: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

    #[test]
    fn the_operations_need_operate_and_check_their_fields() {
        let list = Operation::ListWalletLinks {};
        assert_eq!(list.name(), "wallet.link.list");
        assert_eq!(list.required(), Some(Right::Operate));
        let json = serde_json::to_value(&list).unwrap();
        assert!(json.to_string().contains("wallet.link.list"), "{json}");
        let id = "0f".repeat(16);
        let sealed = Sealed {
            v: "openagents.wallet-link.v1".into(),
            from: KEY.into(),
            payload: "A".repeat(132),
        };
        let answer = Operation::AnswerWalletLink {
            id: id.clone(),
            sealed: Some(sealed.clone()),
        };
        assert_eq!(answer.name(), "wallet.link.answer");
        assert!(answer.validate().is_ok());
        assert!(
            Outcome::WalletLinkAnswered { id: id.clone() }.answers(&answer)
                && !Outcome::WalletLinkAnswered {
                    id: "1f".repeat(16)
                }
                .answers(&answer)
        );
        let bad = Operation::AnswerWalletLink {
            id: "short".into(),
            sealed: None,
        };
        assert!(bad.validate().is_err());
        let mut wrong = sealed;
        wrong.from = "zz".into();
        assert!(wrong.validate().is_err());
        let ask = Ask {
            id,
            key: KEY.into(),
            computer: "studio".into(),
            created_at: 1,
            expires_at: 2,
        };
        assert!(ask.validate().is_ok());
        let too_many = Outcome::WalletLinks {
            links: vec![ask; MAX_LISTED + 1],
        };
        assert!(too_many.validate().is_err());
    }
}
