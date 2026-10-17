//! The browser session a website sign-in with a Nostr key ends in (#12001,
//! [`crate::web_sign_in`]): the account service issued `token`, and this
//! reads it back the way every other sign-in's session is read before it
//! becomes the browser's cookie.

use axum::http::HeaderValue;

use super::{CloudSession, Result, SessionError, SessionGrant, cookie_at, session_token};

impl CloudSession {
    /// The grant for a session `token` the account service just issued to
    /// a website sign-in; `created` when that sign-in made the account.
    pub(crate) async fn web_grant(&self, token: &str, created: bool) -> Result<SessionGrant> {
        if !session_token(token) {
            return Err(SessionError::Unavailable);
        }
        let viewer = self.read_view(token, None).await?;
        Ok(SessionGrant {
            viewer,
            token: jev::ApiKey::new(token),
            secure: self.secure,
            created,
        })
    }

    /// End a session a website sign-in issued that no browser will use.
    pub(crate) async fn end_web_session(&self, token: &str) {
        if let Ok(client) = self.client(token) {
            client.account().sign_out().await.ok();
        }
    }

    /// The cookie that ties one waiting sign-in to the browser that
    /// started it, on `path` only; an empty `value` clears it.
    pub(crate) fn web_sign_in_cookie(
        &self,
        name: &str,
        value: &str,
        seconds: u64,
        path: &str,
    ) -> Result<HeaderValue> {
        cookie_at(name, value, seconds, self.secure, path)
    }
}
