//! WorkOS access tokens: RS256 JWTs signed with the environment's keys,
//! published at `/sso/jwks/{client_id}`. A token is checked here, locally,
//! against a cached key set. A token signed by a key the cache doesn't
//! hold makes one fetch, at most once a minute, so a key rotation is
//! picked up and a flood of bad tokens can't make us hammer WorkOS.
//!
//! Reimplemented from the SDK's sealed-session check (`workos` 3.5.0,
//! `helpers/session.rs`), which only checks tokens inside a sealed cookie.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;

use super::Refusal;

/// The checked claims we read from an access token.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Claims {
    /// The WorkOS User ID.
    pub sub: String,
    /// The WorkOS session ID, which sign-out revokes.
    #[serde(default)]
    pub sid: String,
    /// Expiry, Unix seconds.
    pub exp: i64,
    #[serde(default)]
    pub org_id: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
}

/// The shape a WorkOS access token has: three base64url parts, the first
/// a JSON header. Anything else is never sent to the checks.
#[must_use]
pub fn looks_like_access_token(token: &str) -> bool {
    token.len() <= 8 * 1024
        && token.starts_with("eyJ")
        && token.split('.').count() == 3
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}

/// The environment's key set, cached.
#[derive(Clone)]
pub struct Keys {
    client: Arc<workos::Client>,
    cache: Arc<RwLock<Cache>>,
}

#[derive(Default)]
struct Cache {
    set: Option<Arc<workos::JwkSet>>,
    fetched: Option<Instant>,
}

/// How long a fetched key set is used before it's fetched again.
const TTL: Duration = Duration::from_secs(3600);
/// The least time between two fetches.
const FLOOR: Duration = Duration::from_secs(60);

impl Keys {
    pub(crate) fn new(client: Arc<workos::Client>) -> Self {
        Self {
            client,
            cache: Arc::new(RwLock::new(Cache::default())),
        }
    }

    /// Fetch the key set now, unless it was fetched within the last
    /// minute.
    pub async fn refresh(&self) -> Result<(), Refusal> {
        if self
            .cache
            .read()
            .ok()
            .and_then(|cache| cache.fetched)
            .is_some_and(|at| at.elapsed() < FLOOR)
        {
            return Ok(());
        }
        let set = self
            .client
            .jwks()
            .refresh()
            .await
            .map_err(|_| Refusal::Unavailable)?;
        if let Ok(mut cache) = self.cache.write() {
            cache.set = Some(set);
            cache.fetched = Some(Instant::now());
        }
        Ok(())
    }

    /// Check `token` with the cached keys only, without waiting on the
    /// network. `None` means the cache can't answer (no keys yet, the
    /// token's key unknown, or the keys are old): call [`Self::verify`].
    #[must_use]
    pub fn verify_cached(
        &self,
        token: &str,
        allow_expired: bool,
    ) -> Option<Result<Claims, Refusal>> {
        let cache = self.cache.read().ok()?;
        let set = cache.set.clone()?;
        if cache.fetched.is_none_or(|at| at.elapsed() > TTL) {
            return None;
        }
        drop(cache);
        match check(&set, token, allow_expired) {
            Err(Check::UnknownKey) => None,
            Err(Check::Bad) => Some(Err(Refusal::InvalidCredentials)),
            Ok(claims) => Some(Ok(claims)),
        }
    }

    /// Check `token`, fetching the keys when the cache can't answer.
    /// With `allow_expired`, an expired but genuine token passes (sign-out
    /// uses it to find the session to end).
    pub async fn verify(&self, token: &str, allow_expired: bool) -> Result<Claims, Refusal> {
        if !looks_like_access_token(token) {
            return Err(Refusal::InvalidCredentials);
        }
        if let Some(answer) = self.verify_cached(token, allow_expired) {
            return answer;
        }
        self.refresh().await?;
        let set = self
            .cache
            .read()
            .ok()
            .and_then(|cache| cache.set.clone())
            .ok_or(Refusal::Unavailable)?;
        check(&set, token, allow_expired).map_err(|_| Refusal::InvalidCredentials)
    }

    /// Put a key set in the cache directly (tests and the fake).
    pub fn install(&self, set: workos::JwkSet) {
        if let Ok(mut cache) = self.cache.write() {
            cache.set = Some(Arc::new(set));
            cache.fetched = Some(Instant::now());
        }
    }
}

enum Check {
    UnknownKey,
    Bad,
}

fn check(set: &workos::JwkSet, token: &str, allow_expired: bool) -> Result<Claims, Check> {
    if !looks_like_access_token(token) {
        return Err(Check::Bad);
    }
    let header = decode_header(token).map_err(|_| Check::Bad)?;
    if header.alg != Algorithm::RS256 {
        return Err(Check::Bad);
    }
    let kid = header.kid.filter(|kid| !kid.is_empty()).ok_or(Check::Bad)?;
    let mut matching = set
        .keys
        .iter()
        .filter(|key| key.kid.as_deref() == Some(kid.as_str()));
    let key = matching.next().ok_or(Check::UnknownKey)?;
    if matching.next().is_some()
        || key.kty.as_deref() != Some("RSA")
        || key.alg.as_deref().is_some_and(|alg| alg != "RS256")
        || key.use_.as_deref().is_some_and(|usage| usage != "sig")
    {
        return Err(Check::Bad);
    }
    let jwk: jsonwebtoken::jwk::Jwk = serde_json::to_value(key)
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
        .ok_or(Check::Bad)?;
    let decoding = DecodingKey::from_jwk(&jwk).map_err(|_| Check::Bad)?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_required_spec_claims(&["exp", "sub"]);
    validation.validate_exp = !allow_expired;
    validation.leeway = 0;
    validation.validate_nbf = true;
    // The key set is the environment's own; WorkOS sets no audience.
    validation.validate_aud = false;
    let claims = decode::<Claims>(token, &decoding, &validation)
        .map_err(|_| Check::Bad)?
        .claims;
    if !tenancy::workos_links::valid_user(&claims.sub) {
        return Err(Check::Bad);
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::looks_like_access_token;

    #[test]
    fn only_jwt_shaped_bearers_are_checked() {
        assert!(looks_like_access_token(
            "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJ1In0.c2ln"
        ));
        assert!(!looks_like_access_token("sess_0123"));
        assert!(!looks_like_access_token("eyJ.only-two"));
        assert!(!looks_like_access_token("eyJa.b.c d"));
    }
}
