//! Approving a website login the phone scanned (#12001). The website's
//! `/login` shows a QR code of `{website}/login/scan#request=…&challenge=…`;
//! the phone reads it with [`ScannedLogin::parse`], may show which browser
//! it is for ([`Client::scanned_login`]), and approves it with its key
//! ([`Client::approve_scanned_login`]). The browser then signs in as the
//! key's account. See `crates/openagents-web/src/web_sign_in.rs`.

use super::*;

/// The path a website login's QR code links to.
pub const SCAN_PATH: &str = "/login/scan";

/// A website login the phone scanned. The challenge is a one-time
/// capability: never log it.
#[derive(Clone, PartialEq, Eq)]
pub struct ScannedLogin {
    /// The website that showed the code, `scheme://host[:port]`.
    pub origin: String,
    request: String,
    challenge: String,
}

impl fmt::Debug for ScannedLogin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScannedLogin")
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}

impl ScannedLogin {
    /// Read a scanned code, or `None` when it isn't a website login.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let url = Url::parse(text.trim()).ok()?;
        if url.path() != SCAN_PATH || !crate::transport::allowed_origin(&url) {
            return None;
        }
        let fragment = url.fragment()?;
        let mut request = None;
        let mut challenge = None;
        for (name, value) in url::form_urlencoded::parse(fragment.as_bytes()) {
            match name.as_ref() {
                "request" => request = Some(value.into_owned()),
                "challenge" => challenge = Some(value.into_owned()),
                _ => {}
            }
        }
        let request =
            request.filter(|r| r.len() == 32 && r.bytes().all(|b| b.is_ascii_hexdigit()))?;
        let challenge = challenge.filter(|c| !c.is_empty() && c.len() <= 256)?;
        Some(Self {
            origin: url.origin().ascii_serialization(),
            request,
            challenge,
        })
    }
}

/// Which browser a scanned login is for.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ScannedLoginAbout {
    /// The browser, in a few words ("Chrome on macOS").
    pub browser: String,
    pub expires_at: u64,
}

/// An approved website login.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ScannedLoginApproved {
    pub browser: String,
    /// Whether this login made the account.
    #[serde(default)]
    pub created: bool,
}

impl Client {
    /// Refuse a code from any website but this deployment's.
    fn check_scanned(&self, scanned: &ScannedLogin) -> Result<(), Error> {
        if self.deployment.website().origin().ascii_serialization() == scanned.origin {
            Ok(())
        } else {
            Err(Error::InvalidReference)
        }
    }

    /// Which browser `scanned` signs in, to show before approving.
    pub async fn scanned_login(&self, scanned: &ScannedLogin) -> Result<ScannedLoginAbout, Error> {
        self.check_scanned(scanned)?;
        self.request(
            Method::GET,
            &["web-sign-in", &scanned.request],
            &[],
            None,
            None,
        )
        .await
    }

    /// Approve `scanned` with the key behind `signer`: the browser that
    /// showed the code signs in as that key's account (a new key makes it).
    /// Sends once.
    pub async fn approve_scanned_login(
        &self,
        signer: &dyn NostrSigner,
        scanned: &ScannedLogin,
    ) -> Result<ScannedLoginApproved, Error> {
        self.check_scanned(scanned)?;
        let url = self.api_url(&["web-sign-in", "approve"])?;
        let body = serde_json::to_vec(&json!({"request": scanned.request}))
            .map_err(|_| Error::InvalidCredential)?;
        let authorization = Self::http_auth(signer, &url, &scanned.challenge, Some(&body))?;
        let response = self
            .http
            .post(url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Nostr {authorization}"),
            )
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|error| Error::Network {
                timeout: error.is_timeout(),
            })?;
        match Self::check_response(response).await {
            Ok(response) => Self::decode(response, &["web-sign-in", "approve"], 64 * 1024).await,
            Err(Error::Unauthorized) => Err(Error::KeyDenied),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scanned_website_login_is_read_and_anything_else_is_not() {
        let id = "0123456789abcdef0123456789abcdef";
        let link = format!("https://openagents.com/login/scan#request={id}&challenge=v1.9.ab.cd");
        let scanned = ScannedLogin::parse(&link).unwrap();
        assert_eq!(scanned.origin, "https://openagents.com");
        assert_eq!(scanned.request, id);
        assert_eq!(scanned.challenge, "v1.9.ab.cd");
        assert!(!format!("{scanned:?}").contains("v1.9"));
        assert!(ScannedLogin::parse("https://openagents.com/connect#abc").is_none());
        assert!(ScannedLogin::parse(&link.replace("https", "http")).is_none());
        assert!(ScannedLogin::parse("https://openagents.com/login/scan#request=1").is_none());
    }
}
