//! Sign in on the website by scanning its code (#12001, #12000). The
//! website's login page shows a QR code of `{website}/login/scan#…`; the
//! phone's scanner hands the text here. The app first shows which browser
//! it signs in ([`CoreAccount::scanned_login`]), then approves with the
//! phone's identity key ([`CoreAccount::approve_scanned_login`]). A code
//! from any website but this deployment's is refused.
use super::{CoreAccount, CoreError, CoreResult};
use openagents_account::openagents::{Error, ScannedLogin};

/// Whether `text` is a website login code (any website; the account
/// refuses other sites' codes when asked about one). Dials nothing.
#[uniffi::export]
pub fn is_website_login(text: String) -> bool {
    ScannedLogin::parse(&text).is_some()
}

fn message(error: &Error) -> CoreError {
    let message = match error {
        Error::InvalidReference => {
            "This code is from another website. Scan the code on openagents.com."
        }
        Error::NotFound
        | Error::Refused { status: 404, .. }
        | Error::Refused { status: 410, .. } => {
            "This code has expired. Reload the page on your computer and scan again."
        }
        Error::Refused { status: 409, .. } => {
            "This code was already used. Reload the page on your computer and scan again."
        }
        Error::KeyDenied | Error::InvalidCredential => {
            "That sign-in didn't go through. Reload the page on your computer and scan again."
        }
        Error::Network { .. } => {
            return CoreError::Network {
                message: "Couldn't reach OpenAgents. Check your connection.".into(),
            };
        }
        _ => "Couldn't sign in on the website right now. Try again in a minute.",
    };
    CoreError::HostError {
        message: message.into(),
    }
}

fn not_a_login() -> CoreError {
    CoreError::InvalidArgument {
        message: "This isn't a sign-in code from openagents.com.".into(),
    }
}

#[uniffi::export]
impl CoreAccount {
    /// The browser a scanned website login signs in ("Chrome on macOS"),
    /// to name in the confirm sheet before approving.
    pub async fn scanned_login(&self, code: String) -> CoreResult<String> {
        let scanned = ScannedLogin::parse(&code).ok_or_else(not_a_login)?;
        let client = self.controller().client().clone();
        let task = openagents_client::runtime::shared()
            .spawn(async move { client.scanned_login(&scanned).await });
        match task.await {
            Ok(Ok(about)) => Ok(about.browser),
            Ok(Err(error)) => Err(message(&error)),
            Err(_) => Err(message(&Error::InvalidResponse)),
        }
    }

    /// Approve a scanned website login with this phone's identity key: the
    /// browser that showed the code signs in as this account. Ask first
    /// with [`Self::scanned_login`]. Answers the browser's name.
    pub async fn approve_scanned_login(&self, code: String) -> CoreResult<String> {
        let scanned = ScannedLogin::parse(&code).ok_or_else(not_a_login)?;
        let identity = self
            .identity
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| CoreError::Auth {
                message: "Your account is still starting. Try again in a moment.".into(),
            })?;
        let client = self.controller().client().clone();
        let task = openagents_client::runtime::shared()
            .spawn(async move { client.approve_scanned_login(&*identity, &scanned).await });
        match task.await {
            Ok(Ok(approved)) => Ok(approved.browser),
            Ok(Err(error)) => Err(message(&error)),
            Err(_) => Err(message(&Error::InvalidResponse)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_website_login_codes_are_logins() {
        let id = "0123456789abcdef0123456789abcdef";
        assert!(is_website_login(format!(
            "https://openagents.com/login/scan#request={id}&challenge=v1.9.ab.cd"
        )));
        assert!(!is_website_login(
            "https://openagents.com/connect#AQID".into()
        ));
        assert!(!is_website_login("openagents-connect:AQID".into()));
    }

    #[test]
    fn refusals_read_plainly() {
        let CoreError::HostError { message: other } = message(&Error::InvalidReference) else {
            panic!("another site's code is a plain refusal");
        };
        assert!(other.contains("another website"));
        for error in [Error::InvalidReference, Error::KeyDenied, Error::NotFound] {
            let CoreError::HostError { message } = message(&error) else {
                panic!("plain refusal");
            };
            assert!(oa_copy_ok(&message), "{message}");
        }
    }

    fn oa_copy_ok(text: &str) -> bool {
        !text.is_empty() && !text.contains("challenge") && !text.contains("NIP")
    }
}
