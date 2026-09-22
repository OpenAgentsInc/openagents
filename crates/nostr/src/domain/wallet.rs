//! NIP-60 Cashu wallets.
//!
//! Kind `17375` is a replaceable wallet. Public `mint` tags name the
//! mints. The content is NIP-44 ciphertext addressed to the author.
//! After decryption it holds the wallet private key and the same mint
//! URLs. That key is only for P2PK ecash. It is not the Nostr key.
//!
//! Kind `7375` records unspent proofs. Spending some of them keeps the
//! rest in a new token and deletes the old event with a kind `5` request
//! whose `k` tag is `7375`. Kind `7376` is optional history. Kind `7374`
//! stores a mint quote until it expires.
//!
//! The relay does not decrypt the content, talk to a mint, or check a
//! proof signature. A quote shorter than 14 days is kept. Fourteen days
//! is the maximum, because that is the stated Lightning in-flight bound.
//! NIP-60 is a draft, so these kinds stay off the NIP-11 list.

use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};

use super::deletion::DeletionRequest;
use super::hex::decode_lower_hex;
use super::{DomainError, Event};

const WALLET_KIND: u16 = 17_375;
const TOKEN_KIND: u16 = 7_375;
const HISTORY_KIND: u16 = 7_376;
const QUOTE_KIND: u16 = 7_374;
const TOKEN_DELETION_KIND: &str = "7375";
/// Fourteen days, the longest Lightning payment the spec treats as in flight.
const QUOTE_LIFETIME_SECONDS: u64 = 14 * 24 * 60 * 60;

/// Public mints on a kind `17375` wallet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CashuWallet {
    pub mints: Vec<String>,
}

/// The decrypted wallet record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletSecrets {
    pub private_key: String,
    pub mints: Vec<String>,
}

/// One Cashu proof inside a token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CashuProof {
    pub id: String,
    pub amount: u64,
    pub secret: Option<String>,
    pub commitment: Option<String>,
}

/// Unspent proofs for one mint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CashuToken {
    pub mint: String,
    pub unit: String,
    pub proofs: Vec<CashuProof>,
    pub deleted: Vec<String>,
}

/// Whether a history row received or sent funds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryDirection {
    In,
    Out,
}

/// What an `e` tag says about a token or a nutzap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenRole {
    Created,
    Destroyed,
    Redeemed,
}

/// One token or nutzap named by a history event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenRef {
    pub event_id: String,
    pub relay: Option<String>,
    pub role: TokenRole,
}

/// A kind `7376` spending-history record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpendHistory {
    pub direction: HistoryDirection,
    pub amount: u64,
    pub unit: String,
    pub refs: Vec<TokenRef>,
}

/// A kind `7374` mint quote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MintQuote {
    pub quote_id: String,
    pub mint: String,
    pub expiration: u64,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn xonly(secret: &SecretKey) -> XOnlyPublicKey {
    let secp = Secp256k1::signing_only();
    Keypair::from_secret_key(&secp, secret)
        .x_only_public_key()
        .0
}

fn ciphertext(content: &str) -> Result<(), DomainError> {
    crate::nip44::payload_shape(content).map_err(|_| invalid("wallet content is NIP-44 ciphertext"))
}

fn https_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn relay_url(value: &str) -> bool {
    value.is_empty()
        || ((value.starts_with("wss://") || value.starts_with("ws://"))
            && value.len() > "wss://".len()
            && value.len() <= 2_048
            && !value.chars().any(char::is_whitespace))
}

fn event_id(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "token event id")
        .map_err(|_| invalid("a token event id is 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn unit_name(value: Option<&str>) -> Result<String, DomainError> {
    match value {
        None => Ok("sat".to_owned()),
        Some(value)
            if !value.is_empty()
                && value.len() <= 8
                && value.bytes().all(|byte| byte.is_ascii_lowercase()) =>
        {
            Ok(value.to_owned())
        }
        Some(_) => Err(invalid("a cashu unit is a short lowercase name")),
    }
}

fn whole(value: &str) -> Result<u64, DomainError> {
    if value.is_empty()
        || value == "0"
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid("a cashu amount is a positive integer"));
    }
    value
        .parse()
        .map_err(|_| invalid("a cashu amount is a positive integer"))
}

fn string_rows(plaintext: &str, reason: &str) -> Result<Vec<Vec<String>>, DomainError> {
    let value: serde_json::Value = serde_json::from_str(plaintext).map_err(|_| invalid(reason))?;
    let Some(rows) = value.as_array() else {
        return Err(invalid(reason));
    };
    rows.iter()
        .map(|row| {
            let Some(fields) = row.as_array() else {
                return Err(invalid(reason));
            };
            fields
                .iter()
                .map(|field| {
                    field
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| invalid(reason))
                })
                .collect()
        })
        .collect()
}

fn public_mints(event: &Event) -> Result<Vec<String>, DomainError> {
    let mut mints = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("mint")) {
        let Some(value) = tag.value().filter(|value| https_url(value)) else {
            return Err(invalid("a wallet mint is an http:// or https:// URL"));
        };
        if mints.iter().any(|seen| seen == value) {
            return Err(invalid("a wallet mint is listed once"));
        }
        mints.push(value.to_owned());
    }
    if mints.is_empty() {
        return Err(invalid("a wallet lists one or more mints"));
    }
    Ok(mints)
}

/// Encrypt `plaintext` to `secret` with NIP-44.
pub fn seal_wallet_content(
    secret: &SecretKey,
    plaintext: &str,
    nonce: [u8; 32],
) -> Result<String, DomainError> {
    let key = crate::nip44::conversation_key(secret, &xonly(secret));
    crate::nip44::encrypt(plaintext, &key, nonce)
        .map_err(|_| invalid("wallet content is NIP-44 ciphertext"))
}

/// Decrypt wallet content that was sealed to `secret`.
pub fn read_wallet_content(secret: &SecretKey, payload: &str) -> Result<String, DomainError> {
    let key = crate::nip44::conversation_key(secret, &xonly(secret));
    crate::nip44::decrypt(payload, &key).map_err(|_| invalid("wallet content is NIP-44 ciphertext"))
}

/// Read the public mints of a kind `17375` wallet.
pub fn open_wallet(event: &Event) -> Result<CashuWallet, DomainError> {
    if event.kind != WALLET_KIND {
        return Err(invalid("a cashu wallet has kind 17375"));
    }
    if event.tags.iter().any(|tag| tag.name() == Some("privkey")) {
        return Err(invalid(
            "a wallet private key stays in the encrypted content",
        ));
    }
    ciphertext(&event.content)?;
    Ok(CashuWallet {
        mints: public_mints(event)?,
    })
}

/// Read the decrypted wallet key and check it against the public mints.
pub fn open_wallet_secrets(
    plaintext: &str,
    nostr_secret_hex: &str,
    public_mints: &[String],
) -> Result<WalletSecrets, DomainError> {
    let mut private_key = None;
    let mut mints = Vec::new();
    for row in string_rows(plaintext, "wallet content is a list of pairs")? {
        if row.len() != 2 {
            return Err(invalid("wallet content is a list of pairs"));
        }
        match row[0].as_str() {
            "privkey" => {
                if private_key.is_some() {
                    return Err(invalid("a wallet has one private key"));
                }
                decode_lower_hex::<32>(&row[1], "wallet private key")
                    .map_err(|_| invalid("a wallet private key is 32 lowercase hex bytes"))?;
                if row[1] == nostr_secret_hex {
                    return Err(invalid("a wallet private key is not the nostr key"));
                }
                private_key = Some(row[1].clone());
            }
            "mint" => {
                if !https_url(&row[1]) || mints.iter().any(|seen| seen == &row[1]) {
                    return Err(invalid("a wallet mint is an http:// or https:// URL"));
                }
                mints.push(row[1].clone());
            }
            _ => return Err(invalid("wallet content is a private key and mint URLs")),
        }
    }
    let Some(private_key) = private_key else {
        return Err(invalid("a wallet has one private key"));
    };
    if mints.len() != public_mints.len() || mints.iter().any(|mint| !public_mints.contains(mint)) {
        return Err(invalid("wallet mints match the public mint tags"));
    }
    Ok(WalletSecrets { private_key, mints })
}

/// Admit a kind `7375` token. The proofs stay inside the ciphertext.
pub fn open_token_event(event: &Event) -> Result<(), DomainError> {
    if event.kind != TOKEN_KIND {
        return Err(invalid("a cashu token has kind 7375"));
    }
    ciphertext(&event.content)
}

/// Read decrypted unspent proofs.
pub fn open_token(plaintext: &str) -> Result<CashuToken, DomainError> {
    let value: serde_json::Value =
        serde_json::from_str(plaintext).map_err(|_| invalid("a token is a mint and its proofs"))?;
    let Some(object) = value.as_object() else {
        return Err(invalid("a token is a mint and its proofs"));
    };
    for key in object.keys() {
        if !matches!(key.as_str(), "mint" | "unit" | "proofs" | "del") {
            return Err(invalid("a token is a mint and its proofs"));
        }
    }
    let Some(mint) = object.get("mint").and_then(serde_json::Value::as_str) else {
        return Err(invalid("a token names its mint"));
    };
    if !https_url(mint) {
        return Err(invalid("a token mint is an http:// or https:// URL"));
    }
    let unit = unit_name(object.get("unit").and_then(serde_json::Value::as_str))?;
    let Some(proofs) = object.get("proofs").and_then(serde_json::Value::as_array) else {
        return Err(invalid("a token lists one or more proofs"));
    };
    if proofs.is_empty() {
        return Err(invalid("a token lists one or more proofs"));
    }
    let mut parsed = Vec::new();
    for proof in proofs {
        let Some(fields) = proof.as_object() else {
            return Err(invalid("a proof has an id and an amount"));
        };
        for key in fields.keys() {
            if !matches!(key.as_str(), "id" | "amount" | "secret" | "C") {
                return Err(invalid("a proof has an id and an amount"));
            }
        }
        let Some(id) = fields.get("id").and_then(serde_json::Value::as_str) else {
            return Err(invalid("a proof has an id and an amount"));
        };
        if id.is_empty()
            || id.len() > 128
            || id.chars().any(char::is_whitespace)
            || parsed.iter().any(|seen: &CashuProof| seen.id == id)
        {
            return Err(invalid("a proof id is unique"));
        }
        let Some(amount) = fields.get("amount").and_then(serde_json::Value::as_u64) else {
            return Err(invalid("a proof amount is a positive integer"));
        };
        if amount == 0 {
            return Err(invalid("a proof amount is a positive integer"));
        }
        let secret = match fields.get("secret") {
            None => None,
            Some(value) => {
                let Some(secret) = value.as_str() else {
                    return Err(invalid("a proof secret is text"));
                };
                if secret.is_empty()
                    || secret.chars().any(char::is_whitespace)
                    || secret.len() > 512
                {
                    return Err(invalid("a proof secret is text"));
                }
                Some(secret.to_owned())
            }
        };
        let commitment = match fields.get("C") {
            None => None,
            Some(value) => {
                let Some(commitment) = value.as_str() else {
                    return Err(invalid("a proof commitment is 33 lowercase hex bytes"));
                };
                decode_lower_hex::<33>(commitment, "proof commitment")
                    .map_err(|_| invalid("a proof commitment is 33 lowercase hex bytes"))?;
                Some(commitment.to_owned())
            }
        };
        parsed.push(CashuProof {
            id: id.to_owned(),
            amount,
            secret,
            commitment,
        });
    }
    let mut deleted = Vec::new();
    if let Some(ids) = object.get("del") {
        let Some(ids) = ids.as_array() else {
            return Err(invalid("a token del list is event ids"));
        };
        for id in ids {
            let Some(id) = id.as_str() else {
                return Err(invalid("a token del list is event ids"));
            };
            let id = event_id(id)?;
            if deleted.contains(&id) {
                return Err(invalid("a token del list is event ids"));
            }
            deleted.push(id);
        }
    }
    Ok(CashuToken {
        mint: mint.to_owned(),
        unit,
        proofs: parsed,
        deleted,
    })
}

/// Keep the unspent proofs and record `deleted_event_id` in `del`.
pub fn roll_over_token(
    token: &CashuToken,
    spent_ids: &[&str],
    deleted_event_id: &str,
) -> Result<CashuToken, DomainError> {
    if spent_ids.is_empty() {
        return Err(invalid("a rollover names the spent proofs"));
    }
    let mut spent = Vec::new();
    for id in spent_ids {
        if spent.contains(id) || !token.proofs.iter().any(|proof| proof.id == *id) {
            return Err(invalid("a rollover names proofs from the token"));
        }
        spent.push(*id);
    }
    let proofs: Vec<CashuProof> = token
        .proofs
        .iter()
        .filter(|proof| !spent.contains(&proof.id.as_str()))
        .cloned()
        .collect();
    if proofs.is_empty() {
        return Err(invalid("a rollover keeps the unspent proofs"));
    }
    let deleted_event_id = event_id(deleted_event_id)?;
    let mut deleted = token.deleted.clone();
    if !deleted.contains(&deleted_event_id) {
        deleted.push(deleted_event_id);
    }
    Ok(CashuToken {
        mint: token.mint.clone(),
        unit: token.unit.clone(),
        proofs,
        deleted,
    })
}

/// JSON plaintext for [`seal_wallet_content`].
pub fn token_plaintext(token: &CashuToken) -> String {
    let proofs = token
        .proofs
        .iter()
        .map(|proof| {
            let mut fields = serde_json::Map::new();
            fields.insert("id".to_owned(), serde_json::Value::String(proof.id.clone()));
            fields.insert(
                "amount".to_owned(),
                serde_json::Value::Number(proof.amount.into()),
            );
            if let Some(secret) = &proof.secret {
                fields.insert(
                    "secret".to_owned(),
                    serde_json::Value::String(secret.clone()),
                );
            }
            if let Some(commitment) = &proof.commitment {
                fields.insert(
                    "C".to_owned(),
                    serde_json::Value::String(commitment.clone()),
                );
            }
            serde_json::Value::Object(fields)
        })
        .collect();
    let mut body = serde_json::Map::new();
    body.insert(
        "mint".to_owned(),
        serde_json::Value::String(token.mint.clone()),
    );
    body.insert(
        "unit".to_owned(),
        serde_json::Value::String(token.unit.clone()),
    );
    body.insert("proofs".to_owned(), serde_json::Value::Array(proofs));
    if !token.deleted.is_empty() {
        body.insert(
            "del".to_owned(),
            serde_json::Value::Array(
                token
                    .deleted
                    .iter()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    serde_json::Value::Object(body).to_string()
}

/// A kind `5` deletion of kind `7375` tokens.
pub fn open_token_deletion(event: &Event) -> Result<DeletionRequest, DomainError> {
    let request = DeletionRequest::from_event(event)?;
    let kinds = event.tag_values("k").collect::<Vec<_>>();
    if kinds.len() != 1 || kinds[0] != TOKEN_DELETION_KIND {
        return Err(invalid("a token deletion names kind 7375"));
    }
    if request.event_ids.is_empty() {
        return Err(invalid("a token deletion names the spent token"));
    }
    Ok(request)
}

fn token_role(value: &str) -> Result<TokenRole, DomainError> {
    match value {
        "created" => Ok(TokenRole::Created),
        "destroyed" => Ok(TokenRole::Destroyed),
        "redeemed" => Ok(TokenRole::Redeemed),
        _ => Err(invalid(
            "a history marker is created, destroyed, or redeemed",
        )),
    }
}

fn token_ref(id: &str, relay: &str, role: &str) -> Result<TokenRef, DomainError> {
    if !relay_url(relay) {
        return Err(invalid("a history relay is ws:// or wss://"));
    }
    Ok(TokenRef {
        event_id: event_id(id)?,
        relay: if relay.is_empty() {
            None
        } else {
            Some(relay.to_owned())
        },
        role: token_role(role)?,
    })
}

/// Admit a kind `7376` history event.
///
/// A public `e` tag must use the `redeemed` marker. `created` and
/// `destroyed` stay inside the ciphertext.
pub fn open_history_event(event: &Event) -> Result<Vec<TokenRef>, DomainError> {
    if event.kind != HISTORY_KIND {
        return Err(invalid("a cashu history event has kind 7376"));
    }
    ciphertext(&event.content)?;
    let mut refs = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("e")) {
        let parts = tag.as_slice();
        if parts.len() != 4 {
            return Err(invalid("a public history tag is a redeemed event"));
        }
        if parts[3] != "redeemed" {
            return Err(invalid("a public history tag is a redeemed event"));
        }
        let reference = token_ref(&parts[1], &parts[2], &parts[3])?;
        if refs.iter().any(|seen: &TokenRef| {
            seen.event_id == reference.event_id && seen.role == reference.role
        }) {
            return Err(invalid("a history reference is listed once"));
        }
        refs.push(reference);
    }
    Ok(refs)
}

/// Read decrypted history and keep public `redeemed` tags.
pub fn open_spend_history(plaintext: &str, event: &Event) -> Result<SpendHistory, DomainError> {
    let mut refs = open_history_event(event)?;
    let mut direction = None;
    let mut amount = None;
    let mut unit = None;
    for row in string_rows(plaintext, "history content is a list of pairs")? {
        match row.first().map(String::as_str) {
            Some("direction") if row.len() == 2 => {
                if direction.is_some() {
                    return Err(invalid("history has one direction"));
                }
                direction = Some(match row[1].as_str() {
                    "in" => HistoryDirection::In,
                    "out" => HistoryDirection::Out,
                    _ => return Err(invalid("history direction is in or out")),
                });
            }
            Some("amount") if row.len() == 2 => {
                if amount.is_some() {
                    return Err(invalid("history has one amount"));
                }
                amount = Some(whole(&row[1])?);
            }
            Some("unit") if row.len() == 2 => {
                if unit.is_some() {
                    return Err(invalid("history has one unit"));
                }
                unit = Some(unit_name(Some(&row[1]))?);
            }
            Some("e") if row.len() == 4 => {
                let reference = token_ref(&row[1], &row[2], &row[3])?;
                if refs
                    .iter()
                    .any(|seen| seen.event_id == reference.event_id && seen.role == reference.role)
                {
                    return Err(invalid("a history reference is listed once"));
                }
                refs.push(reference);
            }
            _ => {
                return Err(invalid(
                    "history content is direction, amount, and token ids",
                ));
            }
        }
    }
    let Some(direction) = direction else {
        return Err(invalid("history has one direction"));
    };
    let Some(amount) = amount else {
        return Err(invalid("history has one amount"));
    };
    if refs.is_empty() {
        return Err(invalid(
            "history names a created, destroyed, or redeemed event",
        ));
    }
    Ok(SpendHistory {
        direction,
        amount,
        unit: unit.unwrap_or_else(|| "sat".to_owned()),
        refs,
    })
}

/// Admit a kind `7374` quote. The quote id stays inside the ciphertext.
pub fn open_quote_event(event: &Event) -> Result<(String, u64), DomainError> {
    if event.kind != QUOTE_KIND {
        return Err(invalid("a mint quote has kind 7374"));
    }
    ciphertext(&event.content)?;
    let mints = public_mints(event)?;
    if mints.len() != 1 {
        return Err(invalid("a mint quote names one mint"));
    }
    let expirations: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("expiration"))
        .collect();
    if expirations.len() != 1 {
        return Err(invalid("a mint quote has one expiration"));
    }
    let Some(value) = expirations[0].value() else {
        return Err(invalid("a mint quote expiration is unix seconds"));
    };
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid("a mint quote expiration is unix seconds"));
    }
    let expiration: u64 = value
        .parse()
        .map_err(|_| invalid("a mint quote expiration is unix seconds"))?;
    if expiration <= event.created_at || expiration - event.created_at > QUOTE_LIFETIME_SECONDS {
        return Err(invalid("a mint quote expires within 14 days"));
    }
    Ok((mints[0].clone(), expiration))
}

/// Read a decrypted quote id and the public mint and expiration.
pub fn open_mint_quote(event: &Event, plaintext: &str) -> Result<MintQuote, DomainError> {
    let (mint, expiration) = open_quote_event(event)?;
    if plaintext.is_empty() || plaintext.len() > 256 || plaintext.chars().any(char::is_whitespace) {
        return Err(invalid("a mint quote id is one token"));
    }
    Ok(MintQuote {
        quote_id: plaintext.to_owned(),
        mint,
        expiration,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DomainError, EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement,
    };

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"60".repeat(32)).unwrap()
    }

    fn secret() -> SecretKey {
        SecretKey::from_byte_array([0x60; 32]).unwrap()
    }

    fn seal(plaintext: &str) -> String {
        seal_wallet_content(&secret(), plaintext, [0x60; 32]).unwrap()
    }

    #[test]
    fn a_wallet_replaces_and_a_spent_proof_rolls_into_a_new_token() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/60.md"
        ))
        .unwrap();
        assert!(text.contains("17375"));
        assert!(text.contains("7375"));
        assert!(text.contains("7376"));
        assert!(text.contains("7374"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "60.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "60.md")
        );

        let author = signer();
        let nostr_secret = "60".repeat(32);
        let wallet_key = "61".repeat(32);
        let first_mint = "https://mint.example";
        let second_mint = "https://stablenut.umint.cash";
        let wallet_plain = format!(
            r#"[["privkey","{wallet_key}"],["mint","{first_mint}"],["mint","{second_mint}"]]"#
        );
        let wallet = author.sign(
            1_700_000_000,
            WALLET_KIND,
            vec![
                Tag::new(vec!["mint".into(), first_mint.into()]),
                Tag::new(vec!["mint".into(), second_mint.into()]),
            ],
            seal(&wallet_plain),
        );
        wallet.validate_structure().unwrap();
        assert_eq!(wallet.class(), EventClass::Replaceable);
        let opened = open_wallet(&wallet).unwrap();
        assert_eq!(opened.mints, vec![first_mint, second_mint]);
        let secrets = open_wallet_secrets(
            &read_wallet_content(&secret(), &wallet.content).unwrap(),
            &nostr_secret,
            &opened.mints,
        )
        .unwrap();
        assert_eq!(secrets.private_key, wallet_key);
        assert_ne!(secrets.private_key, nostr_secret);
        assert_eq!(secrets.mints, opened.mints);

        let replaced = author.sign(
            1_700_000_100,
            WALLET_KIND,
            vec![Tag::new(vec!["mint".into(), first_mint.into()])],
            seal(&format!(
                r#"[["privkey","{wallet_key}"],["mint","{first_mint}"]]"#
            )),
        );
        replaced.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&wallet, &replaced),
            Ok(ReplacementDecision::ReplaceCurrent)
        );

        let commitment = "0241d98a8197ef238a192d47edf191a9de78b657308937b4f7dd0aa53beae72c46";
        let token_plain = format!(
            r#"{{"mint":"{second_mint}","unit":"sat","proofs":[{{"id":"1","amount":1}},{{"id":"2","amount":2}},{{"id":"3","amount":4,"secret":"z+zyxAVLRqN9lEjxuNPSyRJzEstbl69Jc1vtimvtkPg=","C":"{commitment}"}},{{"id":"4","amount":8}}]}}"#
        );
        let token_event = author.sign(1_700_000_200, TOKEN_KIND, Vec::new(), seal(&token_plain));
        token_event.validate_structure().unwrap();
        assert_eq!(token_event.class(), EventClass::Regular);
        let token =
            open_token(&read_wallet_content(&secret(), &token_event.content).unwrap()).unwrap();
        assert_eq!(token.proofs.len(), 4);
        assert_eq!(token.proofs[2].commitment.as_deref(), Some(commitment));
        let rolled = roll_over_token(&token, &["3"], &token_event.id).unwrap();
        assert_eq!(
            rolled
                .proofs
                .iter()
                .map(|proof| proof.amount)
                .collect::<Vec<_>>(),
            vec![1, 2, 8]
        );
        assert_eq!(rolled.deleted, vec![token_event.id.clone()]);
        let rolled_event = author.sign(
            1_700_000_300,
            TOKEN_KIND,
            Vec::new(),
            seal(&token_plaintext(&rolled)),
        );
        rolled_event.validate_structure().unwrap();
        let stored =
            open_token(&read_wallet_content(&secret(), &rolled_event.content).unwrap()).unwrap();
        assert_eq!(stored, rolled);
        assert!(matches!(
            compare_replacement(&token_event, &rolled_event),
            Err(DomainError::NotReplaceable)
        ));

        let deletion = author.sign(
            1_700_000_400,
            5,
            vec![
                Tag::new(vec!["e".into(), token_event.id.clone()]),
                Tag::new(vec!["k".into(), "7375".into()]),
            ],
            String::new(),
        );
        deletion.validate_structure().unwrap();
        let request = open_token_deletion(&deletion).unwrap();
        assert!(request.deletes(&token_event));
        assert!(!request.deletes(&rolled_event));

        let nutzap = "ab".repeat(32);
        let history_plain = format!(
            r#"[["direction","out"],["amount","4"],["e","{}","","destroyed"],["e","{}","","created"]]"#,
            token_event.id, rolled_event.id
        );
        let history = author.sign(
            1_700_000_500,
            HISTORY_KIND,
            vec![Tag::new(vec![
                "e".into(),
                nutzap.clone(),
                String::new(),
                "redeemed".into(),
            ])],
            seal(&history_plain),
        );
        history.validate_structure().unwrap();
        let spent = open_spend_history(
            &read_wallet_content(&secret(), &history.content).unwrap(),
            &history,
        )
        .unwrap();
        assert_eq!(spent.direction, HistoryDirection::Out);
        assert_eq!(spent.amount, 4);
        assert_eq!(spent.unit, "sat");
        assert!(
            spent.refs.iter().any(|item| {
                item.event_id == token_event.id && item.role == TokenRole::Destroyed
            })
        );
        assert!(
            spent.refs.iter().any(|item| {
                item.event_id == rolled_event.id && item.role == TokenRole::Created
            })
        );
        assert!(
            spent
                .refs
                .iter()
                .any(|item| { item.event_id == nutzap && item.role == TokenRole::Redeemed })
        );

        let leaked = author.sign(
            1_700_000_550,
            HISTORY_KIND,
            vec![Tag::new(vec![
                "e".into(),
                token_event.id.clone(),
                String::new(),
                "created".into(),
            ])],
            seal(&history_plain),
        );
        assert!(leaked.validate_structure().is_err());

        let quote_at = 1_700_000_600_u64;
        let quote = author.sign(
            quote_at,
            QUOTE_KIND,
            vec![
                Tag::new(vec!["mint".into(), second_mint.into()]),
                Tag::new(vec![
                    "expiration".into(),
                    (quote_at + QUOTE_LIFETIME_SECONDS).to_string(),
                ]),
            ],
            seal("quote-id"),
        );
        quote.validate_structure().unwrap();
        let opened_quote = open_mint_quote(
            &quote,
            &read_wallet_content(&secret(), &quote.content).unwrap(),
        )
        .unwrap();
        assert_eq!(opened_quote.quote_id, "quote-id");
        assert_eq!(opened_quote.mint, second_mint);
        assert_eq!(opened_quote.expiration, quote_at + QUOTE_LIFETIME_SECONDS);

        let public_key = author.sign(
            1_700_000_700,
            WALLET_KIND,
            vec![
                Tag::new(vec!["mint".into(), first_mint.into()]),
                Tag::new(vec!["privkey".into(), wallet_key.clone()]),
            ],
            seal(&format!(
                r#"[["privkey","{wallet_key}"],["mint","{first_mint}"]]"#
            )),
        );
        assert!(public_key.validate_structure().is_err());
    }
}
