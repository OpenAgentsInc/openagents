//! AP2 (Agent Payments Protocol, v0.2) mandates on our checkout (#11145).
//!
//! A shopping agent buying Pro or credits for a person presents a
//! **Checkout Mandate**: an SD-JWT (`dc+sd-jwt`, ES256) that the person's
//! platform signed over the checkout we signed, optionally key-bound to
//! the agent (`cnf` plus a `kb+sd-jwt`). We verify it before completing
//! (UCP's `dev.ucp.common.payment.ap2_mandate` extension), check a
//! **Payment Mandate** when one rides in the payment credential, and
//! answer with a signed **Checkout Receipt**.
//!
//! What is checked, in order (AP2 "Merchant" verification and the UCP
//! extension's steps):
//!
//! 1. The SD-JWT parses, its issuer JWT is ES256 and verifies against a
//!    key in the platform's profile (`keys`, chosen by `kid`).
//! 2. Every disclosure is referenced by a digest exactly once, and the
//!    claims resolve (`_sd`, `...` array elements, `sha-256` only).
//! 3. With a `cnf` key, a key-binding JWT is required: `kb+sd-jwt`,
//!    signed by that key, `sd_hash` over the presentation, a fresh
//!    `iat`, an `aud` we accept, a nonce.
//! 4. `exp` hasn't passed.
//! 5. The checkout inside is ours: either AP2's `checkout_jwt` (our
//!    compact JWS, with `checkout_hash` matching it) or UCP's embedded
//!    `checkout` whose `ap2.merchant_authorization` (our detached JWS
//!    over the JCS form without `ap2`) verifies.
//! 6. Its terms (id, currency, line items, totals) equal the session's.
//!
//! Any failure is a refusal with the UCP error code; nothing completes.
//! Open mandates (constraints for autonomous spend, chained to a closed
//! one) are not accepted here yet: an open mandate is refused rather than
//! half-checked.

use std::collections::{BTreeMap, BTreeSet};

use ring::rand::{SecureRandom, SystemRandom};
use serde_json::{Map, Value, json};

use super::jose::{self, Jws, PublicKey, Signer};

/// The A2A agent-card extension URI for AP2, as the AP2 reference agents
/// declare it.
pub const AP2_EXTENSION: &str = "https://github.com/google-agentic-commerce/ap2/v1";
/// The A2A agent-card extension URI for x402 (`a2a-x402` v0.2).
pub const X402_EXTENSION: &str = "https://github.com/google-agentic-commerce/a2a-x402/blob/main/spec/v0.2";
/// The UCP capability name of the AP2 mandates extension.
pub const UCP_CAPABILITY: &str = "dev.ucp.common.payment.ap2_mandate";
/// A closed Checkout Mandate's `vct`.
pub const CHECKOUT_VCT: &str = "mandate.checkout.1";
/// An open Checkout Mandate's `vct` (refused for now).
pub const OPEN_CHECKOUT_VCT: &str = "mandate.checkout.open.1";
/// A closed Payment Mandate's `vct`.
pub const PAYMENT_VCT: &str = "mandate.payment.1";
/// How far a key-binding JWT's `iat` may be from now, in seconds.
const KB_WINDOW: u64 = 300;
/// The largest presentation we read.
const MAX_LEN: usize = 64 * 1024;

/// Why a mandate was refused: the UCP AP2 extension's error codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// AP2 is in force and no `ap2.checkout_mandate` came.
    MandateRequired,
    /// The platform's profile names no key we can use.
    AgentMissingKey,
    /// The mandate (or its key binding) doesn't verify, or is malformed.
    InvalidSignature(String),
    /// `exp` has passed.
    Expired,
    /// The mandate is for another checkout or other terms.
    ScopeMismatch(String),
    /// Our own signature inside the mandate doesn't verify.
    MerchantAuthorizationInvalid,
    /// The embedded checkout carries no `ap2.merchant_authorization`.
    MerchantAuthorizationMissing,
}

impl Refusal {
    /// The UCP error code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::MandateRequired => "mandate_required",
            Self::AgentMissingKey => "agent_missing_key",
            Self::InvalidSignature(_) => "mandate_invalid_signature",
            Self::Expired => "mandate_expired",
            Self::ScopeMismatch(_) => "mandate_scope_mismatch",
            Self::MerchantAuthorizationInvalid => "merchant_authorization_invalid",
            Self::MerchantAuthorizationMissing => "merchant_authorization_missing",
        }
    }

    /// One plain sentence for the agent.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::MandateRequired => {
                "This checkout needs a checkout mandate (`ap2.checkout_mandate`) to complete.".into()
            }
            Self::AgentMissingKey => {
                "Your platform profile (the UCP-Agent header) lists no ES256 key to check the mandate with.".into()
            }
            Self::InvalidSignature(why) => format!("The mandate doesn't verify: {why}."),
            Self::Expired => "The mandate has expired.".into(),
            Self::ScopeMismatch(why) => format!("The mandate is for a different checkout: {why}."),
            Self::MerchantAuthorizationInvalid => {
                "The checkout inside the mandate isn't the one we signed.".into()
            }
            Self::MerchantAuthorizationMissing => {
                "The checkout inside the mandate has no `ap2.merchant_authorization`.".into()
            }
        }
    }
}

fn invalid(why: &str) -> Refusal {
    Refusal::InvalidSignature(why.to_owned())
}

/// An SD-JWT presentation, split.
#[derive(Debug, Clone)]
pub struct Presentation {
    pub issuer: Jws,
    pub disclosures: Vec<String>,
    pub kb: Option<Jws>,
    /// `<issuer-jwt>~<disclosure>~…~`: what `sd_hash` covers.
    pub hashed: String,
    /// The whole presentation as received.
    pub raw: String,
}

impl Presentation {
    /// Split `<jwt>~<d1>~…~<dN>~[<kb-jwt>]` (a trailing `~` with nothing
    /// after it, or a final KB-JWT; a form without the trailing `~` is
    /// read the same way).
    ///
    /// # Errors
    ///
    /// The text isn't an SD-JWT with an ES256 issuer JWT.
    pub fn parse(text: &str) -> Result<Self, Refusal> {
        let text = text.trim();
        if text.is_empty() || text.len() > MAX_LEN {
            return Err(invalid("it isn't an SD-JWT"));
        }
        let parts: Vec<&str> = text.split('~').collect();
        let issuer = Jws::parse(parts[0]).ok_or_else(|| invalid("the issuer JWT isn't ES256"))?;
        let mut rest = &parts[1..];
        let mut kb = None;
        let mut hashed_end = text.len();
        if let Some(last) = rest.last() {
            if last.is_empty() {
                rest = &rest[..rest.len() - 1];
            } else if last.matches('.').count() == 2 {
                kb = Some(Jws::parse(last).ok_or_else(|| invalid("the key binding isn't ES256"))?);
                hashed_end = text.len() - last.len();
                rest = &rest[..rest.len() - 1];
            }
        }
        if rest.iter().any(|d| d.is_empty()) {
            return Err(invalid("a disclosure is empty"));
        }
        let mut hashed = text[..hashed_end].to_owned();
        if !hashed.ends_with('~') {
            hashed.push('~');
        }
        Ok(Self {
            issuer,
            disclosures: rest.iter().map(|d| (*d).to_owned()).collect(),
            kb,
            hashed,
            raw: text.to_owned(),
        })
    }

    /// The reference AP2 receipts carry: `base64url(sha256(presentation))`,
    /// computed the way `sd_hash` is.
    #[must_use]
    pub fn reference(&self) -> String {
        jose::sha256_b64url(self.raw.as_bytes())
    }
}

/// Resolve selectively disclosed claims (SD-JWT, `sha-256` only).
///
/// # Errors
///
/// A disclosure is malformed, repeated, unreferenced, or would overwrite
/// a claim.
pub fn disclose(claims: &Map<String, Value>, disclosures: &[String]) -> Result<Map<String, Value>, Refusal> {
    if let Some(alg) = claims.get("_sd_alg")
        && alg != "sha-256"
    {
        return Err(invalid("only sha-256 disclosures are read"));
    }
    let mut by_digest: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for disclosure in disclosures {
        let bytes = jose::unb64url(disclosure).ok_or_else(|| invalid("a disclosure isn't base64url"))?;
        let array: Vec<Value> =
            serde_json::from_slice(&bytes).map_err(|_| invalid("a disclosure isn't a JSON array"))?;
        if !(2..=3).contains(&array.len()) || !array[0].is_string() {
            return Err(invalid("a disclosure isn't [salt, name, value] or [salt, value]"));
        }
        if by_digest
            .insert(jose::sha256_b64url(disclosure.as_bytes()), array)
            .is_some()
        {
            return Err(invalid("a disclosure is repeated"));
        }
    }
    let mut used = BTreeSet::new();
    let mut root = Value::Object(claims.clone());
    resolve(&mut root, &by_digest, &mut used)?;
    if used.len() != by_digest.len() {
        return Err(invalid("a disclosure isn't referenced by the credential"));
    }
    let mut out = match root {
        Value::Object(map) => map,
        _ => return Err(invalid("the claims aren't an object")),
    };
    out.remove("_sd_alg");
    Ok(out)
}

fn resolve(
    value: &mut Value,
    by_digest: &BTreeMap<String, Vec<Value>>,
    used: &mut BTreeSet<String>,
) -> Result<(), Refusal> {
    match value {
        Value::Object(map) => {
            let digests = match map.remove("_sd") {
                None => Vec::new(),
                Some(Value::Array(digests)) => digests,
                Some(_) => return Err(invalid("`_sd` isn't an array")),
            };
            for (_, child) in map.iter_mut() {
                resolve(child, by_digest, used)?;
            }
            for digest in digests {
                let digest = digest.as_str().ok_or_else(|| invalid("a digest isn't a string"))?;
                let Some(disclosure) = by_digest.get(digest) else {
                    continue;
                };
                if !used.insert(digest.to_owned()) {
                    return Err(invalid("a digest appears twice"));
                }
                let [_, name, claim] = disclosure.as_slice() else {
                    return Err(invalid("an object disclosure has no name"));
                };
                let name = name.as_str().ok_or_else(|| invalid("a disclosed name isn't a string"))?;
                if matches!(name, "_sd" | "...") || map.contains_key(name) {
                    return Err(invalid("a disclosure would overwrite a claim"));
                }
                let mut claim = claim.clone();
                resolve(&mut claim, by_digest, used)?;
                map.insert(name.to_owned(), claim);
            }
        }
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for mut item in std::mem::take(items) {
                let reference = item
                    .as_object()
                    .filter(|o| o.len() == 1)
                    .and_then(|o| o.get("..."))
                    .map(|d| d.as_str().map(str::to_owned));
                match reference {
                    Some(None) => return Err(invalid("an array digest isn't a string")),
                    Some(Some(digest)) => {
                        let Some(disclosure) = by_digest.get(&digest) else {
                            continue;
                        };
                        if !used.insert(digest) {
                            return Err(invalid("a digest appears twice"));
                        }
                        let [_, claim] = disclosure.as_slice() else {
                            return Err(invalid("an array disclosure has a name"));
                        };
                        let mut claim = claim.clone();
                        resolve(&mut claim, by_digest, used)?;
                        out.push(claim);
                    }
                    None => {
                        resolve(&mut item, by_digest, used)?;
                        out.push(item);
                    }
                }
            }
            *items = out;
        }
        _ => {}
    }
    Ok(())
}

/// What a verifier needs besides the mandate.
pub struct Verifier<'a> {
    /// The platform's keys (its UCP profile's `keys`).
    pub platform: &'a [PublicKey],
    /// Our keys: what `checkout_jwt` and `merchant_authorization` verify
    /// against.
    pub merchant: &'a [PublicKey],
    /// `aud` values a key-binding JWT may name (our origin, our profile).
    pub audiences: &'a [String],
    pub now: u64,
}

/// A verified mandate.
#[derive(Debug, Clone)]
pub struct Verified {
    /// The resolved claims.
    pub claims: Map<String, Value>,
    /// `base64url(sha256(presentation))`, for the receipt.
    pub reference: String,
    /// What a Payment Mandate's `transaction_id` must equal.
    pub transaction: String,
}

impl Verifier<'_> {
    /// Steps 1–4: signature, disclosures, key binding, expiry.
    fn credential(&self, text: &str) -> Result<(Presentation, Map<String, Value>), Refusal> {
        if self.platform.is_empty() {
            return Err(Refusal::AgentMissingKey);
        }
        let presentation = Presentation::parse(text)?;
        let key = jose::pick(self.platform, presentation.issuer.kid()).ok_or(Refusal::AgentMissingKey)?;
        if !presentation.issuer.verified_by(key) {
            return Err(invalid("the platform's signature doesn't match"));
        }
        if let Some(typ) = presentation.issuer.typ()
            && !typ.ends_with("sd-jwt")
        {
            return Err(invalid("the issuer JWT isn't an SD-JWT"));
        }
        let signed = presentation
            .issuer
            .claims()
            .ok_or_else(|| invalid("the issuer JWT's claims aren't an object"))?;
        let claims = disclose(&signed, &presentation.disclosures)?;
        if let Some(cnf) = claims.get("cnf") {
            let holder = PublicKey::from_jwk(&cnf["jwk"])
                .ok_or_else(|| invalid("the `cnf` key isn't an ES256 JWK"))?;
            let kb = presentation
                .kb
                .as_ref()
                .ok_or_else(|| invalid("the key-bound mandate has no key-binding JWT"))?;
            if kb.typ() != Some("kb+sd-jwt") || !kb.verified_by(&holder) {
                return Err(invalid("the key-binding JWT doesn't verify"));
            }
            let kb_claims = kb
                .claims()
                .ok_or_else(|| invalid("the key-binding claims aren't an object"))?;
            if kb_claims.get("sd_hash").and_then(Value::as_str)
                != Some(jose::sha256_b64url(presentation.hashed.as_bytes()).as_str())
            {
                return Err(invalid("the key binding is for another presentation"));
            }
            let iat = kb_claims.get("iat").and_then(Value::as_u64).unwrap_or(0);
            if iat + KB_WINDOW < self.now || iat > self.now + 60 {
                return Err(invalid("the key binding isn't fresh"));
            }
            let aud_ok = kb_claims
                .get("aud")
                .and_then(Value::as_str)
                .is_some_and(|aud| self.audiences.iter().any(|a| a == aud));
            if !aud_ok {
                return Err(invalid("the key binding is for another merchant"));
            }
            if kb_claims
                .get("nonce")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                return Err(invalid("the key binding has no nonce"));
            }
        } else if presentation.kb.is_some() {
            return Err(invalid("a key binding came without a `cnf` key"));
        }
        if let Some(exp) = claims.get("exp") {
            let exp = exp.as_u64().ok_or_else(|| invalid("`exp` isn't a time"))?;
            if exp <= self.now {
                return Err(Refusal::Expired);
            }
        }
        Ok((presentation, claims))
    }

    /// Verify a closed Checkout Mandate against the checkout we hold
    /// (`current`: the checkout as we answer it, without `ap2`).
    ///
    /// # Errors
    ///
    /// The refusal names the UCP error code.
    pub fn checkout_mandate(&self, text: &str, current: &Value) -> Result<Verified, Refusal> {
        let (presentation, claims) = self.credential(text)?;
        match claims.get("vct").and_then(Value::as_str) {
            None | Some(CHECKOUT_VCT) => {}
            Some(OPEN_CHECKOUT_VCT) => {
                return Err(Refusal::ScopeMismatch(
                    "open mandates aren't accepted here yet; present a closed one".into(),
                ));
            }
            Some(_) => return Err(invalid("it isn't a checkout mandate")),
        }
        let (embedded, transaction) = if let Some(jwt) = claims.get("checkout_jwt") {
            let jwt = jwt.as_str().ok_or_else(|| invalid("`checkout_jwt` isn't a string"))?;
            let hash = jose::sha256_b64url(jwt.as_bytes());
            if claims.get("checkout_hash").and_then(Value::as_str) != Some(hash.as_str()) {
                return Err(invalid("`checkout_hash` doesn't match `checkout_jwt`"));
            }
            let jws = Jws::parse(jwt).ok_or(Refusal::MerchantAuthorizationInvalid)?;
            let key = jose::pick(self.merchant, jws.kid()).ok_or(Refusal::MerchantAuthorizationInvalid)?;
            if !jws.verified_by(key) {
                return Err(Refusal::MerchantAuthorizationInvalid);
            }
            let checkout = jws.claims().ok_or(Refusal::MerchantAuthorizationInvalid)?;
            (Value::Object(checkout), hash)
        } else if let Some(checkout) = claims.get("checkout") {
            let authorization = checkout["ap2"]["merchant_authorization"]
                .as_str()
                .ok_or(Refusal::MerchantAuthorizationMissing)?;
            let mut content = checkout.clone();
            if let Some(map) = content.as_object_mut() {
                map.remove("ap2");
            }
            if !jose::verify_detached(authorization, &content, self.merchant) {
                return Err(Refusal::MerchantAuthorizationInvalid);
            }
            let hash = jose::sha256_b64url(jose::canonical(checkout).as_bytes());
            (content, hash)
        } else {
            return Err(Refusal::ScopeMismatch("it carries no checkout".into()));
        };
        same_terms(&embedded, current)?;
        Ok(Verified {
            claims,
            reference: presentation.reference(),
            transaction,
        })
    }

    /// Verify a closed Payment Mandate: the same credential checks, then
    /// that it pays this checkout (`transaction_id`) the exact total to us.
    ///
    /// # Errors
    ///
    /// The refusal names the UCP error code.
    pub fn payment_mandate(
        &self,
        text: &str,
        transaction: &str,
        amount: u64,
        currency: &str,
    ) -> Result<Verified, Refusal> {
        let (presentation, claims) = self.credential(text)?;
        if claims.get("vct").and_then(Value::as_str) != Some(PAYMENT_VCT) {
            return Err(Refusal::ScopeMismatch(
                "the payment credential isn't a closed payment mandate".into(),
            ));
        }
        if claims.get("transaction_id").and_then(Value::as_str) != Some(transaction) {
            return Err(Refusal::ScopeMismatch(
                "the payment mandate is for another checkout".into(),
            ));
        }
        let paid = &claims["payment_amount"];
        if paid["amount"].as_u64() != Some(amount)
            || !paid["currency"]
                .as_str()
                .is_some_and(|c| c.eq_ignore_ascii_case(currency))
        {
            return Err(Refusal::ScopeMismatch(
                "the payment mandate's amount isn't this checkout's total".into(),
            ));
        }
        if claims.get("payee").is_none_or(Value::is_null) {
            return Err(Refusal::ScopeMismatch("the payment mandate names no payee".into()));
        }
        Ok(Verified {
            reference: presentation.reference(),
            transaction: transaction.to_owned(),
            claims,
        })
    }
}

/// The terms that must not change between what the person approved and
/// what completes.
fn same_terms(embedded: &Value, current: &Value) -> Result<(), Refusal> {
    for field in ["id", "currency", "line_items", "totals"] {
        if embedded.get(field) != current.get(field) {
            return Err(Refusal::ScopeMismatch(format!("its `{field}` differs")));
        }
    }
    Ok(())
}

/// A signed AP2 Checkout Receipt (a compact JWT): `Success` with the
/// order, or `Error` with the code.
///
/// # Errors
///
/// Signing failed.
pub fn receipt(
    signer: &Signer,
    issuer: &str,
    reference: &str,
    outcome: Result<&str, &Refusal>,
    now: u64,
) -> Result<String, String> {
    let mut claims = json!({
        "iss": issuer,
        "iat": now,
        "reference": reference,
    });
    match outcome {
        Ok(order) => {
            claims["status"] = json!("Success");
            claims["order_id"] = json!(order);
        }
        Err(refusal) => {
            claims["status"] = json!("Error");
            claims["error"] = json!(refusal.code());
            claims["error_description"] = json!(refusal.message());
        }
    }
    let mut header = Map::new();
    header.insert("typ".into(), json!("JWT"));
    signer.compact(&header, jose::canonical(&claims).as_bytes())
}

/// The agent card's AP2 extension: we act as the merchant, and checkout
/// (with mandates) is at our UCP profile.
#[must_use]
pub fn agent_card_extension(profile: &str) -> Value {
    json!({
        "uri": AP2_EXTENSION,
        "description": "Buy the Pro plan or API credits for a person with an AP2 checkout mandate, through our UCP checkout.",
        "required": false,
        "params": {
            "roles": ["merchant"],
            "ucp_profile": profile,
            "mandates": [CHECKOUT_VCT, PAYMENT_VCT],
            "vp_formats_supported": {"dc+sd-jwt": {}},
        },
    })
}

/// The agent card's x402 extension: the API takes x402 per request.
#[must_use]
pub fn x402_card_extension(api: &str) -> Value {
    json!({
        "uri": X402_EXTENSION,
        "description": "The OpenAgents API takes x402 payments per request, with no key.",
        "required": false,
        "params": {"api": api},
    })
}

// --- Issuing mandates: the buyer side (the phone, a platform) and tests. ---

fn salt() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "no randomness for a salt".to_owned())?;
    Ok(jose::b64url(&bytes))
}

/// Issue an SD-JWT: `always` claims in the clear, `disclosed` ones
/// selectively disclosable (all disclosed in the result), and the
/// holder's key as `cnf` when the mandate is key-bound. Ends with `~`.
///
/// # Errors
///
/// Signing failed.
pub fn issue(
    signer: &Signer,
    always: Map<String, Value>,
    disclosed: Vec<(String, Value)>,
    holder: Option<&PublicKey>,
) -> Result<String, String> {
    let mut claims = always;
    let mut digests = Vec::new();
    let mut parts = Vec::new();
    for (name, value) in disclosed {
        let disclosure = jose::b64url(
            serde_json::to_string(&json!([salt()?, name, value]))
                .map_err(|e| e.to_string())?
                .as_bytes(),
        );
        digests.push(json!(jose::sha256_b64url(disclosure.as_bytes())));
        parts.push(disclosure);
    }
    if !digests.is_empty() {
        claims.insert("_sd".into(), Value::Array(digests));
        claims.insert("_sd_alg".into(), json!("sha-256"));
    }
    if let Some(holder) = holder {
        claims.insert("cnf".into(), json!({"jwk": holder.jwk()}));
    }
    let mut header = Map::new();
    header.insert("typ".into(), json!("dc+sd-jwt"));
    let jwt = signer.compact(&header, jose::canonical(&Value::Object(claims)).as_bytes())?;
    let mut out = jwt;
    for part in parts {
        out.push('~');
        out.push_str(&part);
    }
    out.push('~');
    Ok(out)
}

/// Bind an issued SD-JWT (ending in `~`) to the holder for one verifier.
///
/// # Errors
///
/// Signing failed.
pub fn bind(holder: &Signer, sd_jwt: &str, aud: &str, nonce: &str, now: u64) -> Result<String, String> {
    let claims = json!({
        "iat": now,
        "aud": aud,
        "nonce": nonce,
        "sd_hash": jose::sha256_b64url(sd_jwt.as_bytes()),
    });
    let mut header = Map::new();
    header.insert("typ".into(), json!("kb+sd-jwt"));
    let kb = holder.compact(&header, jose::canonical(&claims).as_bytes())?;
    Ok(format!("{sd_jwt}{kb}"))
}

/// A closed Checkout Mandate over a checkout we answered (the UCP form:
/// the checkout, with its `ap2.merchant_authorization`, disclosed).
///
/// # Errors
///
/// Signing failed.
pub fn checkout_mandate(
    platform: &Signer,
    checkout: &Value,
    holder: Option<&PublicKey>,
    exp: u64,
) -> Result<String, String> {
    let mut always = Map::new();
    always.insert("vct".into(), json!(CHECKOUT_VCT));
    always.insert("exp".into(), json!(exp));
    issue(
        platform,
        always,
        vec![("checkout".into(), checkout.clone())],
        holder,
    )
}

/// A closed Payment Mandate for `transaction` (the checkout's hash).
///
/// # Errors
///
/// Signing failed.
pub fn payment_mandate(
    platform: &Signer,
    transaction: &str,
    payee: &Value,
    amount: u64,
    currency: &str,
    exp: u64,
) -> Result<String, String> {
    let mut always = Map::new();
    always.insert("vct".into(), json!(PAYMENT_VCT));
    always.insert("exp".into(), json!(exp));
    always.insert("transaction_id".into(), json!(transaction));
    issue(
        platform,
        always,
        vec![
            ("payee".into(), payee.clone()),
            (
                "payment_amount".into(),
                json!({"amount": amount, "currency": currency}),
            ),
            (
                "payment_instrument".into(),
                json!({"type": "card", "description": "Card through Stripe Checkout"}),
            ),
        ],
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_791_590_400;

    fn checkout(merchant: &Signer) -> Value {
        let mut checkout = json!({
            "id": "chk_1",
            "status": "ready_for_complete",
            "currency": "USD",
            "line_items": [{"id": "li_1", "item": {"id": "pro", "title": "Pro", "price": 2000}, "quantity": 1}],
            "totals": [{"type": "subtotal", "amount": 2000}, {"type": "total", "amount": 2000}],
        });
        let authorization = merchant.detached(&checkout).unwrap();
        checkout["ap2"] = json!({"merchant_authorization": authorization});
        checkout
    }

    fn without_ap2(checkout: &Value) -> Value {
        let mut current = checkout.clone();
        current.as_object_mut().unwrap().remove("ap2");
        current
    }

    struct Keys {
        merchant: Signer,
        platform: Signer,
        holder: Signer,
    }

    fn keys() -> Keys {
        Keys {
            merchant: Signer::generate("merchant-1").unwrap().0,
            platform: Signer::generate("platform-1").unwrap().0,
            holder: Signer::generate("agent-1").unwrap().0,
        }
    }

    fn verify(k: &Keys, mandate: &str, current: &Value) -> Result<Verified, Refusal> {
        let audiences = vec!["https://api.openagents.com".to_owned()];
        let platform = [k.platform.public()];
        let merchant = [k.merchant.public()];
        Verifier {
            platform: &platform,
            merchant: &merchant,
            audiences: &audiences,
            now: NOW,
        }
        .checkout_mandate(mandate, current)
    }

    #[test]
    fn a_sample_mandate_verifies() {
        let k = keys();
        let checkout = checkout(&k.merchant);
        let issued = checkout_mandate(&k.platform, &checkout, None, NOW + 600).unwrap();
        let verified = verify(&k, &issued, &without_ap2(&checkout)).unwrap();
        assert_eq!(verified.claims["vct"], CHECKOUT_VCT);
        assert_eq!(verified.claims["checkout"]["id"], "chk_1");
    }

    #[test]
    fn a_key_bound_mandate_verifies_and_needs_its_binding() {
        let k = keys();
        let checkout = checkout(&k.merchant);
        let issued =
            checkout_mandate(&k.platform, &checkout, Some(&k.holder.public()), NOW + 600).unwrap();
        // Without the key binding: refused.
        assert!(matches!(
            verify(&k, &issued, &without_ap2(&checkout)),
            Err(Refusal::InvalidSignature(_))
        ));
        let bound = bind(&k.holder, &issued, "https://api.openagents.com", "n-1", NOW).unwrap();
        assert!(verify(&k, &bound, &without_ap2(&checkout)).is_ok());
        // Bound for another merchant: refused.
        let elsewhere = bind(&k.holder, &issued, "https://other.example", "n-1", NOW).unwrap();
        assert!(verify(&k, &elsewhere, &without_ap2(&checkout)).is_err());
        // Bound by another key: refused.
        let (thief, _) = Signer::generate("agent-1").unwrap();
        let stolen = bind(&thief, &issued, "https://api.openagents.com", "n-1", NOW).unwrap();
        assert!(verify(&k, &stolen, &without_ap2(&checkout)).is_err());
    }

    #[test]
    fn a_tampered_mandate_is_refused() {
        let k = keys();
        let checkout = checkout(&k.merchant);
        let issued = checkout_mandate(&k.platform, &checkout, None, NOW + 600).unwrap();
        let current = without_ap2(&checkout);

        // A changed signed payload.
        let mut parts: Vec<String> = issued.split('~').map(str::to_owned).collect();
        let jwt: Vec<&str> = parts[0].split('.').collect();
        let mut claims: Value =
            serde_json::from_slice(&jose::unb64url(jwt[1]).unwrap()).unwrap();
        claims["exp"] = json!(NOW + 999_999);
        let forged = format!(
            "{}.{}.{}",
            jwt[0],
            jose::b64url(jose::canonical(&claims).as_bytes()),
            jwt[2]
        );
        parts[0] = forged;
        assert!(matches!(
            verify(&k, &parts.join("~"), &current),
            Err(Refusal::InvalidSignature(_))
        ));

        // A swapped disclosure: a cheaper checkout the platform never signed.
        let mut cheaper = checkout.clone();
        cheaper["totals"][1]["amount"] = json!(1);
        let disclosure = jose::b64url(
            serde_json::to_string(&json!(["salt", "checkout", cheaper]))
                .unwrap()
                .as_bytes(),
        );
        let mut swapped: Vec<String> = issued.split('~').map(str::to_owned).collect();
        swapped[1] = disclosure;
        assert!(matches!(
            verify(&k, &swapped.join("~"), &current),
            Err(Refusal::InvalidSignature(_))
        ));

        // A platform signature over a checkout we never signed.
        let mut unsigned = checkout.clone();
        unsigned["totals"][1]["amount"] = json!(1);
        let issued = checkout_mandate(&k.platform, &unsigned, None, NOW + 600).unwrap();
        assert_eq!(
            verify(&k, &issued, &current).unwrap_err(),
            Refusal::MerchantAuthorizationInvalid
        );

        // A mandate for another checkout's terms.
        let mut other = current.clone();
        other["id"] = json!("chk_2");
        let issued = checkout_mandate(&k.platform, &checkout, None, NOW + 600).unwrap();
        assert!(matches!(
            verify(&k, &issued, &other),
            Err(Refusal::ScopeMismatch(_))
        ));

        // Expired.
        let issued = checkout_mandate(&k.platform, &checkout, None, NOW - 1).unwrap();
        assert_eq!(verify(&k, &issued, &current).unwrap_err(), Refusal::Expired);

        // Signed by a key the platform's profile doesn't list.
        let (stranger, _) = Signer::generate("platform-1").unwrap();
        let issued = checkout_mandate(&stranger, &checkout, None, NOW + 600).unwrap();
        assert!(matches!(
            verify(&k, &issued, &current),
            Err(Refusal::InvalidSignature(_))
        ));

        // No merchant authorization inside.
        let issued = checkout_mandate(&k.platform, &current, None, NOW + 600).unwrap();
        assert_eq!(
            verify(&k, &issued, &current).unwrap_err(),
            Refusal::MerchantAuthorizationMissing
        );
    }

    #[test]
    fn the_ap2_checkout_jwt_form_verifies_too() {
        let k = keys();
        let current = without_ap2(&checkout(&k.merchant));
        let jwt = k
            .merchant
            .compact(&Map::new(), jose::canonical(&current).as_bytes())
            .unwrap();
        let mut always = Map::new();
        always.insert("vct".into(), json!(CHECKOUT_VCT));
        always.insert("checkout_hash".into(), json!(jose::sha256_b64url(jwt.as_bytes())));
        let issued = issue(&k.platform, always.clone(), vec![("checkout_jwt".into(), json!(jwt))], None).unwrap();
        let verified = verify(&k, &issued, &current).unwrap();
        assert_eq!(verified.transaction, jose::sha256_b64url(jwt.as_bytes()));
        // A hash for some other checkout JWT.
        always.insert("checkout_hash".into(), json!("AAAA"));
        let issued = issue(&k.platform, always, vec![("checkout_jwt".into(), json!(jwt))], None).unwrap();
        assert!(verify(&k, &issued, &current).is_err());
    }

    #[test]
    fn open_mandates_are_refused_for_now() {
        let k = keys();
        let mut always = Map::new();
        always.insert("vct".into(), json!(OPEN_CHECKOUT_VCT));
        let issued = issue(&k.platform, always, vec![], None).unwrap();
        assert!(matches!(
            verify(&k, &issued, &json!({})),
            Err(Refusal::ScopeMismatch(_))
        ));
    }

    #[test]
    fn a_payment_mandate_must_pay_this_checkout_its_total() {
        let k = keys();
        let platform = [k.platform.public()];
        let merchant = [k.merchant.public()];
        let audiences: Vec<String> = vec![];
        let verifier = Verifier {
            platform: &platform,
            merchant: &merchant,
            audiences: &audiences,
            now: NOW,
        };
        let payee = json!({"id": "openagents", "name": "OpenAgents"});
        let good = payment_mandate(&k.platform, "tx-1", &payee, 2000, "USD", NOW + 60).unwrap();
        assert!(verifier.payment_mandate(&good, "tx-1", 2000, "USD").is_ok());
        assert!(verifier.payment_mandate(&good, "tx-2", 2000, "USD").is_err());
        assert!(verifier.payment_mandate(&good, "tx-1", 1999, "USD").is_err());
    }

    #[test]
    fn disclosures_must_all_be_referenced_once() {
        let claims: Map<String, Value> = serde_json::from_value(json!({"a": 1})).unwrap();
        let stray = jose::b64url(br#"["s","b",2]"#);
        assert!(disclose(&claims, &[stray.clone()]).is_err());
        let mut referenced = claims.clone();
        referenced.insert("_sd".into(), json!([jose::sha256_b64url(stray.as_bytes())]));
        let out = disclose(&referenced, &[stray.clone()]).unwrap();
        assert_eq!(out["b"], 2);
        assert!(disclose(&referenced, &[stray.clone(), stray]).is_err());
    }

    #[test]
    fn receipts_are_signed_and_name_the_outcome() {
        let k = keys();
        let ok = receipt(&k.merchant, "https://api.openagents.com", "ref", Ok("cko_1"), NOW).unwrap();
        let jws = Jws::parse(&ok).unwrap();
        assert!(jws.verified_by(&k.merchant.public()));
        assert_eq!(jws.claims().unwrap()["status"], "Success");
        let err = receipt(&k.merchant, "x", "ref", Err(&Refusal::Expired), NOW).unwrap();
        assert_eq!(Jws::parse(&err).unwrap().claims().unwrap()["error"], "mandate_expired");
    }
}
