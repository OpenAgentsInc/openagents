//! Connect your phone (#11998, NIP-HOST "Showing a connect code"): the method
//! names and values the app's Connect your phone window and the engine
//! share.
//!
//! Only this computer's own app reaches these methods: the engine serves
//! them on its in-process and localhost transports, never to another
//! device through the relay or a paired phone. The window asks for a new
//! code every [`ROTATE_EVERY_SECS`], cancels the code it replaced
//! [`CANCEL_REPLACED_AFTER_SECS`] later, and cancels every code it still
//! holds when it closes.

use serde::{Deserialize, Serialize};

/// Make a new connect code. Params `{}`. Returns a [`ConnectCode`].
pub const CREATE_CONNECT_CODE: &str = "CreateConnectCode";
/// Cancel connect codes by invitation ID. Params [`CancelConnectCodes`].
/// Returns `{}`. A code that was redeemed or already cancelled is skipped.
pub const CANCEL_CONNECT_CODES: &str = "CancelConnectCodes";

/// How often the window shows a new code.
pub const ROTATE_EVERY_SECS: u64 = 60;
/// How long a replaced code stays redeemable after the new one appears.
pub const CANCEL_REPLACED_AFTER_SECS: u64 = 60;
/// How long the window keeps showing codes without a click before it stops
/// and cancels them.
pub const IDLE_STOP_SECS: u64 = 600;

/// One connect code for the window to show.
///
/// `link` is what the QR code shows; `text` is what the copy action
/// copies. Both carry a single-use bearer secret: never log either.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectCode {
    /// The invitation ID, which [`CANCEL_CONNECT_CODES`] takes.
    pub invitation: String,
    /// `https://openagents.com/connect#<payload>`.
    pub link: String,
    /// `openagents-connect:<payload>`.
    pub text: String,
    /// Unix seconds.
    pub issued_at: u64,
    /// Unix seconds: the host refuses the code after this.
    pub expires_at: u64,
    /// The computer's name the phone shows.
    pub label: String,
    /// The desktop's account-link request: a signed request naming this
    /// computer's key, which the phone approves so the server adds the key
    /// to the person's account (#12000). `None` until #12000 fills it; the
    /// phone pairs without it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_link: Option<serde_json::Value>,
}

impl std::fmt::Debug for ConnectCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The link and text carry the bearer capability.
        f.debug_struct("ConnectCode")
            .field("invitation", &self.invitation)
            .field("issued_at", &self.issued_at)
            .field("expires_at", &self.expires_at)
            .field("label", &self.label)
            .field("account_link", &self.account_link.is_some())
            .finish_non_exhaustive()
    }
}

/// Params of [`CANCEL_CONNECT_CODES`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelConnectCodes {
    pub invitations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_leaves_out_the_secret() {
        let code = ConnectCode {
            invitation: "i".into(),
            link: "https://openagents.com/connect#SECRET".into(),
            text: "openagents-connect:SECRET".into(),
            issued_at: 1,
            expires_at: 301,
            label: "Mac".into(),
            account_link: None,
        };
        assert!(!format!("{code:?}").contains("SECRET"));
        let json = serde_json::to_value(&code).unwrap();
        assert!(json.get("accountLink").is_none());
        assert_eq!(serde_json::from_value::<ConnectCode>(json).unwrap(), code);
    }
}
