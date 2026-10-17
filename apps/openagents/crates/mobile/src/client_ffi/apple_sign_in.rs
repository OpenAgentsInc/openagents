//! One-tap sign-in on the phone (#11980): Sign in with Apple and the
//! in-app sign-in sheet. Swift shows Apple's sheet and
//! `ASWebAuthenticationSession`; the nonce, the sheet's address, and the
//! token exchange live here, through the same account controller as every
//! other sign-in.
use super::CoreAccount;
use openagents_account::openagents::AppleSignIn;
use sha2::{Digest, Sha256};

/// A Sign in with Apple nonce: `hashed` goes into Apple's request,
/// `raw` comes back to us with the identity token.
#[derive(Clone, Debug, uniffi::Record)]
pub struct AppleNonce {
    pub raw: String,
    pub hashed: String,
}

/// A fresh random nonce for one Sign in with Apple request.
#[uniffi::export]
pub fn apple_sign_in_nonce() -> AppleNonce {
    let raw = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let hashed = Sha256::digest(raw.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    AppleNonce { raw, hashed }
}

/// The scheme the website sends the in-app sign-in sheet back to after
/// Approve or Deny (`openagents://signed-in`).
pub const SIGN_IN_SHEET_SCHEME: &str = "openagents";

/// The approval page to open in the in-app sign-in sheet: the code's page
/// with `return=app`, so Approve sends the sheet back and it closes.
#[uniffi::export]
pub fn sign_in_sheet_url(verification_url: String) -> String {
    let separator = if verification_url.contains('?') {
        '&'
    } else {
        '?'
    };
    format!("{verification_url}{separator}return=app")
}

/// The scheme [`sign_in_sheet_url`]'s page returns to.
#[uniffi::export]
pub fn sign_in_sheet_scheme() -> String {
    SIGN_IN_SHEET_SCHEME.into()
}

#[uniffi::export]
impl CoreAccount {
    /// Sign in with the identity token from Apple's sheet and the raw
    /// nonce from [`apple_sign_in_nonce`]. `name` is the name the sheet
    /// returned (only on the first sign-in). Progress arrives as account
    /// snapshots, like [`CoreAccount::sign_in`].
    pub fn sign_in_with_apple(
        &self,
        identity_token: String,
        nonce: String,
        name: Option<String>,
        device_name: String,
    ) {
        let task = self.controller().sign_in_with_apple(AppleSignIn {
            identity_token,
            nonce,
            name,
            app: String::new(),
            computer: device_name,
        });
        self.close_clients();
        openagents_client::runtime::shared().spawn(task);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{AccountListener, AccountSnapshot, AccountState};
    use openagents_account::openagents::{
        Client, CredentialRecord, CredentialStore, Deployment, StorageError,
    };
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Memory(Mutex<Option<CredentialRecord>>);
    impl CredentialStore for Memory {
        fn load(&self, _: &Deployment) -> Result<Option<CredentialRecord>, StorageError> {
            Ok(self.0.lock().unwrap().clone())
        }
        fn save(&self, record: &CredentialRecord) -> Result<(), StorageError> {
            *self.0.lock().unwrap() = Some(record.clone());
            Ok(())
        }
        fn clear(&self, _: &Deployment) -> Result<(), StorageError> {
            *self.0.lock().unwrap() = None;
            Ok(())
        }
    }
    struct Silent;
    impl AccountListener for Silent {
        fn on_account_changed(&self, _: AccountSnapshot) {}
    }

    /// The token exchange stores the session through the same controller,
    /// so the phone is signed in and its credential saved.
    #[tokio::test]
    async fn apple_sign_in_signs_the_account_in_and_saves_it() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let log = seen.clone();
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let log = log.clone();
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut buf = [0; 4096];
                    loop {
                        let n = socket.read(&mut buf).await.unwrap();
                        request.extend_from_slice(&buf[..n]);
                        let text = String::from_utf8_lossy(&request);
                        let done = text.split_once("\r\n\r\n").is_some_and(|(head, body)| {
                            head.lines()
                                .find_map(|l| {
                                    l.to_ascii_lowercase()
                                        .strip_prefix("content-length: ")
                                        .map(|v| v.trim().parse::<usize>().unwrap())
                                })
                                .is_none_or(|len| body.len() >= len)
                        });
                        if n == 0 || done {
                            break;
                        }
                    }
                    let text = String::from_utf8_lossy(&request).to_string();
                    let body = if text.starts_with("POST /v1/device/apple ") {
                        log.lock().unwrap().push(text.clone());
                        format!(
                            r#"{{"access_token":"sess_{}","token_type":"Bearer","expires_in":2592000,"account":{{"id":"acct_apple","label":"Pat"}}}}"#,
                            "c".repeat(64)
                        )
                    } else {
                        r#"{"account":{"id":"acct_apple","label":"Pat"},"workspaces":[]}"#
                            .to_string()
                    };
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(Memory::default());
        let account = CoreAccount::from_store(
            Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap(),
            store.clone(),
            root.path().into(),
            Arc::new(Silent),
        );
        let nonce = apple_sign_in_nonce();
        account
            .controller()
            .sign_in_with_apple(AppleSignIn {
                identity_token: "header.payload.signature".into(),
                nonce: nonce.raw.clone(),
                name: Some("Pat".into()),
                app: String::new(),
                computer: "Pat's iPhone".into(),
            })
            .await;
        assert!(matches!(
            account.snapshot().state,
            AccountState::SignedIn { ref account_id, .. } if account_id == "acct_apple"
        ));
        assert!(store.0.lock().unwrap().is_some());
        let sent = seen.lock().unwrap().join("");
        assert!(sent.contains(&nonce.raw), "{sent}");
        assert!(sent.contains("\"app\":\"OpenAgents\""), "{sent}");
    }

    #[test]
    fn nonces_are_fresh_and_hashed() {
        let one = apple_sign_in_nonce();
        let two = apple_sign_in_nonce();
        assert_ne!(one.raw, two.raw);
        assert_eq!(one.raw.len(), 64);
        assert_eq!(one.hashed.len(), 64);
        assert_eq!(
            one.hashed,
            Sha256::digest(one.raw.as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
    }

    #[test]
    fn the_sheet_page_asks_to_return_to_the_app() {
        assert_eq!(
            sign_in_sheet_url("https://openagents.com/device?code=BCDF-GHJK".into()),
            "https://openagents.com/device?code=BCDF-GHJK&return=app"
        );
        assert_eq!(
            sign_in_sheet_url("https://openagents.com/device".into()),
            "https://openagents.com/device?return=app"
        );
        assert_eq!(sign_in_sheet_scheme(), "openagents");
    }
}
