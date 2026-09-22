//! NIP-90 data vending machines.
//!
//! Kinds `5000` through `5999` are job requests. The matching result is
//! that kind plus `1000`. Kind `7000` is feedback. An `i` tag is `url`,
//! `event`, `job`, or `text`. A result's `request` tag is the signed
//! request, and its kind and `e` tag must name that request.
//!
//! The relay does not run the job, fetch inputs, or pay invoices. An
//! `encrypted` payload is checked as NIP-04 framing and is not decrypted.
//! These kinds are not added to the NIP-11 list.

use super::hex::decode_lower_hex;
use super::{DomainError, Event};

const REQUEST_START: u16 = 5_000;
const RESULT_START: u16 = 6_000;
const FEEDBACK_KIND: u16 = 7_000;

/// How an `i` tag should be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobInputKind {
    Url,
    Event,
    Job,
    Text,
}

/// One job input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobInput {
    pub data: String,
    pub kind: JobInputKind,
    pub relay: String,
    pub marker: Option<String>,
}

/// One `param` key and value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobParam {
    pub key: String,
    pub value: String,
}

/// A kind `5000`–`5999` job request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobRequest {
    pub kind: u16,
    pub customer: String,
    pub content: String,
    pub inputs: Vec<JobInput>,
    pub output: Option<String>,
    pub params: Vec<JobParam>,
    pub bid_msat: Option<u64>,
    pub relays: Vec<String>,
    pub providers: Vec<String>,
    pub encrypted: bool,
}

/// A kind `6000`–`6999` job result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobResult {
    pub kind: u16,
    pub provider: String,
    pub customer: String,
    pub request_id: String,
    pub request_kind: u16,
    pub content: String,
    pub amount_msat: Option<u64>,
    pub invoice: Option<String>,
    pub encrypted: bool,
}

/// Feedback status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobStatus {
    PaymentRequired,
    Processing,
    Error,
    Success,
    Partial,
}

/// A kind `7000` job feedback event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobFeedback {
    pub status: JobStatus,
    pub detail: Option<String>,
    pub customer: String,
    pub request_id: String,
    pub content: String,
    pub amount_msat: Option<u64>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn http_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn media_type_ok(value: &str) -> bool {
    value.len() <= 127
        && value == value.to_ascii_lowercase()
        && value.split_once('/').is_some_and(|(top, subtype)| {
            !top.is_empty()
                && !subtype.is_empty()
                && top
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&byte))
                && subtype
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&byte))
        })
}

fn nip04_framed(content: &str) -> bool {
    let Some((body, iv)) = content.split_once("?iv=") else {
        return false;
    };
    let alphabet = |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=');
    !body.is_empty() && !iv.is_empty() && body.bytes().all(alphabet) && iv.bytes().all(alphabet)
}

fn millisats(value: &str) -> Result<u64, DomainError> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(invalid("a job amount is millisatoshis"));
    }
    value
        .parse()
        .map_err(|_| invalid("a job amount is millisatoshis"))
}

fn pubkey(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "job pubkey")
        .map_err(|_| invalid("a job pubkey is 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn event_id(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "job event")
        .map_err(|_| invalid("a job event id is 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn tags<'a>(event: &'a Event, name: &str) -> Vec<&'a super::Tag> {
    event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect()
}

/// The result kind for a request kind, `request + 1000`.
#[must_use]
pub fn job_result_kind(request: u16) -> Option<u16> {
    (REQUEST_START..RESULT_START)
        .contains(&request)
        .then_some(request + 1_000)
}

fn encrypted_marker(event: &Event) -> Result<bool, DomainError> {
    match tags(event, "encrypted").as_slice() {
        [] => Ok(false),
        [tag] if tag.as_slice().len() == 1 => Ok(true),
        _ => Err(invalid("a job encrypted tag is a marker")),
    }
}

fn inputs(event: &Event) -> Result<Vec<JobInput>, DomainError> {
    let mut inputs = Vec::new();
    for tag in tags(event, "i") {
        let slice = tag.as_slice();
        let Some(data) = slice
            .get(1)
            .filter(|value| !value.is_empty() && value.len() <= 65_536)
        else {
            return Err(invalid("a job input has data"));
        };
        let kind = match slice.get(2).map(String::as_str) {
            Some("url") => JobInputKind::Url,
            Some("event") => JobInputKind::Event,
            Some("job") => JobInputKind::Job,
            Some("text") => JobInputKind::Text,
            _ => return Err(invalid("a job input type is url, event, job, or text")),
        };
        if kind == JobInputKind::Url && !http_url(data) {
            return Err(invalid("a job url input is http:// or https://"));
        }
        if matches!(kind, JobInputKind::Event | JobInputKind::Job) {
            event_id(data)?;
        }
        let relay = match slice.get(3) {
            None => String::new(),
            Some(value) if value.is_empty() || is_relay(value) => value.to_owned(),
            Some(_) => return Err(invalid("a job input relay is empty or ws:// or wss://")),
        };
        let marker = match slice.get(4) {
            None => None,
            Some(value)
                if !value.is_empty()
                    && value.len() <= 128
                    && !value.chars().any(char::is_control) =>
            {
                Some(value.to_owned())
            }
            Some(_) => return Err(invalid("a job input marker is 1 to 128 characters")),
        };
        inputs.push(JobInput {
            data: data.to_owned(),
            kind,
            relay,
            marker,
        });
    }
    Ok(inputs)
}

fn params(event: &Event) -> Result<Vec<JobParam>, DomainError> {
    let mut params = Vec::new();
    for tag in tags(event, "param") {
        let slice = tag.as_slice();
        let Some(key) = slice.get(1).filter(|key| {
            !key.is_empty() && key.len() <= 64 && !key.chars().any(char::is_whitespace)
        }) else {
            return Err(invalid("a job parameter has a key"));
        };
        let Some(value) = slice
            .get(2)
            .filter(|value| !value.is_empty() && value.len() <= 1_024)
        else {
            return Err(invalid("a job parameter has a value"));
        };
        params.push(JobParam {
            key: key.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(params)
}

fn relay_list(event: &Event) -> Result<Vec<String>, DomainError> {
    let mut relays = Vec::new();
    for tag in tags(event, "relays") {
        let values = tag.as_slice().iter().skip(1).collect::<Vec<_>>();
        if values.is_empty() {
            return Err(invalid("a job relay is ws:// or wss://"));
        }
        for value in values {
            if !is_relay(value) {
                return Err(invalid("a job relay is ws:// or wss://"));
            }
            relays.push(value.to_owned());
        }
    }
    Ok(relays)
}

fn providers(event: &Event) -> Result<Vec<String>, DomainError> {
    tags(event, "p")
        .into_iter()
        .map(|tag| {
            let Some(value) = tag.value() else {
                return Err(invalid("a job pubkey is 32 lowercase hex bytes"));
            };
            pubkey(value)
        })
        .collect()
}

fn one_amount(event: &Event) -> Result<Option<(u64, Option<String>)>, DomainError> {
    match tags(event, "amount").as_slice() {
        [] => Ok(None),
        [tag] => {
            let Some(value) = tag.value() else {
                return Err(invalid("a job amount is millisatoshis"));
            };
            let amount = millisats(value)?;
            let invoice = match tag.as_slice().get(2) {
                None => None,
                Some(invoice) => {
                    if invoice.is_empty() {
                        return Err(invalid("a job invoice is a bolt11 string"));
                    }
                    match super::zap::bolt11_amount_msat(invoice)? {
                        Some(invoice_amount) if invoice_amount != amount => {
                            return Err(invalid("a job invoice amount matches the millisatoshis"));
                        }
                        _ => Some(invoice.to_owned()),
                    }
                }
            };
            Ok(Some((amount, invoice)))
        }
        _ => Err(invalid("a job has one amount")),
    }
}

fn require_framed(event: &Event, encrypted: bool) -> Result<(), DomainError> {
    if !encrypted {
        return Ok(());
    }
    if tags(event, "p").is_empty() || !nip04_framed(&event.content) {
        return Err(invalid(
            "an encrypted job names a pubkey and NIP-04 content",
        ));
    }
    Ok(())
}

/// Read a kind `5000`–`5999` job request.
///
/// # Errors
///
/// Returns a sentence when an input, parameter, bid, or relay is refused.
pub fn open_job_request(event: &Event) -> Result<JobRequest, DomainError> {
    if !(REQUEST_START..RESULT_START).contains(&event.kind) {
        return Err(invalid("a job request has a kind from 5000 through 5999"));
    }
    let encrypted = encrypted_marker(event)?;
    require_framed(event, encrypted)?;
    let output = match tags(event, "output").as_slice() {
        [] => None,
        [tag] => {
            let Some(value) = tag.value().filter(|value| media_type_ok(value)) else {
                return Err(invalid("a job output is one lowercase MIME type"));
            };
            Some(value.to_owned())
        }
        _ => return Err(invalid("a job output is one lowercase MIME type")),
    };
    let bid_msat = match tags(event, "bid").as_slice() {
        [] => None,
        [tag] => {
            let Some(value) = tag.value() else {
                return Err(invalid("a job amount is millisatoshis"));
            };
            Some(millisats(value)?)
        }
        _ => return Err(invalid("a job has one bid")),
    };
    Ok(JobRequest {
        kind: event.kind,
        customer: event.pubkey.clone(),
        content: event.content.clone(),
        inputs: inputs(event)?,
        output,
        params: params(event)?,
        bid_msat,
        relays: relay_list(event)?,
        providers: providers(event)?,
        encrypted,
    })
}

/// Read a kind `6000`–`6999` job result.
///
/// # Errors
///
/// Returns a sentence when the embedded request, customer, or amount is refused.
pub fn open_job_result(event: &Event) -> Result<JobResult, DomainError> {
    if !(RESULT_START..FEEDBACK_KIND).contains(&event.kind) {
        return Err(invalid("a job result has a kind from 6000 through 6999"));
    }
    let encrypted = encrypted_marker(event)?;
    require_framed(event, encrypted)?;
    if encrypted && !tags(event, "i").is_empty() {
        return Err(invalid("an encrypted job result omits cleartext inputs"));
    }
    let requests = tags(event, "request");
    if requests.len() != 1 {
        return Err(invalid("a job result embeds one request"));
    }
    let Some(body) = requests[0].value() else {
        return Err(invalid("a job result embeds one request"));
    };
    let embedded: Event =
        serde_json::from_str(body).map_err(|_| invalid("a job result embeds one request"))?;
    embedded
        .validate_crypto()
        .map_err(|_| invalid("a job result embeds a signed request"))?;
    let request = open_job_request(&embedded)?;
    if job_result_kind(request.kind) != Some(event.kind) {
        return Err(invalid("a job result kind is the request kind plus 1000"));
    }
    let events = tags(event, "e");
    if events.len() != 1 {
        return Err(invalid("a job result names the request"));
    }
    let Some(request_id) = events[0].value() else {
        return Err(invalid("a job result names the request"));
    };
    if request_id != embedded.id {
        return Err(invalid("a job result names the request"));
    }
    if let Some(relay) = events[0].as_slice().get(2)
        && !relay.is_empty()
        && !is_relay(relay)
    {
        return Err(invalid("a job relay is ws:// or wss://"));
    }
    let customers = tags(event, "p");
    if customers.len() != 1 {
        return Err(invalid("a job result names the customer"));
    }
    let Some(customer) = customers[0].value() else {
        return Err(invalid("a job result names the customer"));
    };
    if customer != embedded.pubkey {
        return Err(invalid("a job result names the customer"));
    }
    let (amount_msat, invoice) = match one_amount(event)? {
        None => (None, None),
        Some((amount, invoice)) => (Some(amount), invoice),
    };
    if !encrypted {
        let _inputs = inputs(event)?;
    }
    Ok(JobResult {
        kind: event.kind,
        provider: event.pubkey.clone(),
        customer: customer.to_owned(),
        request_id: request_id.to_owned(),
        request_kind: request.kind,
        content: event.content.clone(),
        amount_msat,
        invoice,
        encrypted,
    })
}

/// Read a kind `7000` job feedback event.
///
/// # Errors
///
/// Returns a sentence when the status, request, or customer is refused.
pub fn open_job_feedback(event: &Event) -> Result<JobFeedback, DomainError> {
    if event.kind != FEEDBACK_KIND {
        return Err(invalid("job feedback has kind 7000"));
    }
    let encrypted = encrypted_marker(event)?;
    require_framed(event, encrypted)?;
    let statuses = tags(event, "status");
    if statuses.len() != 1 {
        return Err(invalid(
            "job feedback status is payment-required, processing, error, success, or partial",
        ));
    }
    let status = match statuses[0].value() {
        Some("payment-required") => JobStatus::PaymentRequired,
        Some("processing") => JobStatus::Processing,
        Some("error") => JobStatus::Error,
        Some("success") => JobStatus::Success,
        Some("partial") => JobStatus::Partial,
        _ => {
            return Err(invalid(
                "job feedback status is payment-required, processing, error, success, or partial",
            ));
        }
    };
    let detail = match statuses[0].as_slice().get(2) {
        None => None,
        Some(value) if !value.is_empty() && value.len() <= 1_024 => Some(value.to_owned()),
        Some(_) => return Err(invalid("job feedback detail is non-empty")),
    };
    let events = tags(event, "e");
    if events.len() != 1 {
        return Err(invalid("job feedback names the request"));
    }
    let Some(request_id) = events[0].value() else {
        return Err(invalid("job feedback names the request"));
    };
    event_id(request_id)?;
    let customers = tags(event, "p");
    if customers.len() != 1 {
        return Err(invalid("job feedback names the customer"));
    }
    let Some(customer) = customers[0].value() else {
        return Err(invalid("job feedback names the customer"));
    };
    let amount_msat = one_amount(event)?.map(|(amount, _)| amount);
    Ok(JobFeedback {
        status,
        detail,
        customer: pubkey(customer)?,
        request_id: request_id.to_owned(),
        content: event.content.clone(),
        amount_msat,
    })
}

#[cfg(test)]
mod tests {
    use secp256k1::{SecretKey, XOnlyPublicKey};

    use super::*;
    use crate::domain::{DomainError, EventClass, RelaySigner, Tag, compare_replacement};
    use crate::nip04;

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    #[test]
    fn a_job_result_uses_the_request_kind_plus_one_thousand() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/90.md"
        ))
        .unwrap();
        assert!(text.contains("kind:5001"));
        assert!(text.contains("5000-5999"));
        assert!(text.contains("6000-6999"));
        assert!(text.contains("7000"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "90.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "90.md")
        );
        assert_eq!(job_result_kind(5_001), Some(6_001));
        assert_eq!(job_result_kind(6_001), None);

        let customer = signer("90");
        let provider = signer("91");
        let source = customer.sign(1_700_000_000, 1, Vec::new(), "audio".into());
        let request = customer.sign(
            1_700_000_100,
            5_001,
            vec![
                Tag::new(vec!["i".into(), "transcribe this".into(), "text".into()]),
                Tag::new(vec![
                    "i".into(),
                    "https://cdn.example/talk.mp3".into(),
                    "url".into(),
                ]),
                Tag::new(vec![
                    "i".into(),
                    source.id.clone(),
                    "event".into(),
                    "wss://relay.example".into(),
                    "source".into(),
                ]),
                Tag::new(vec!["output".into(), "text/plain".into()]),
                Tag::new(vec!["param".into(), "lang".into(), "es".into()]),
                Tag::new(vec!["bid".into(), "21000".into()]),
                Tag::new(vec!["relays".into(), "wss://relay.example".into()]),
                Tag::new(vec!["p".into(), provider.pubkey().to_owned()]),
            ],
            String::new(),
        );
        request.validate_structure().unwrap();
        assert_eq!(request.class(), EventClass::Regular);
        let opened = open_job_request(&request).unwrap();
        assert_eq!(opened.inputs.len(), 3);
        assert_eq!(opened.inputs[0].kind, JobInputKind::Text);
        assert_eq!(opened.output.as_deref(), Some("text/plain"));
        assert_eq!(opened.bid_msat, Some(21_000));
        assert_eq!(opened.params[0].value, "es");
        let later = customer.sign(1_700_000_200, 5_001, Vec::new(), String::new());
        assert!(matches!(
            compare_replacement(&request, &later),
            Err(DomainError::NotReplaceable)
        ));

        let result = provider.sign(
            1_700_000_300,
            6_001,
            vec![
                Tag::new(vec![
                    "request".into(),
                    serde_json::to_string(&request).unwrap(),
                ]),
                Tag::new(vec![
                    "e".into(),
                    request.id.clone(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec!["p".into(), customer.pubkey().to_owned()]),
                Tag::new(vec!["amount".into(), "21000".into()]),
                Tag::new(vec!["i".into(), "transcribe this".into(), "text".into()]),
            ],
            "hola".into(),
        );
        result.validate_structure().unwrap();
        let result = open_job_result(&result).unwrap();
        assert_eq!(result.request_kind, 5_001);
        assert_eq!(result.request_id, request.id);
        assert_eq!(result.customer, customer.pubkey());
        assert_eq!(result.amount_msat, Some(21_000));
        assert_eq!(result.content, "hola");

        let feedback = provider.sign(
            1_700_000_250,
            FEEDBACK_KIND,
            vec![
                Tag::new(vec![
                    "status".into(),
                    "payment-required".into(),
                    "21000 millisatoshis".into(),
                ]),
                Tag::new(vec!["e".into(), request.id.clone()]),
                Tag::new(vec!["p".into(), customer.pubkey().to_owned()]),
                Tag::new(vec!["amount".into(), "21000".into()]),
            ],
            String::new(),
        );
        feedback.validate_structure().unwrap();
        let feedback = open_job_feedback(&feedback).unwrap();
        assert_eq!(feedback.status, JobStatus::PaymentRequired);
        assert_eq!(feedback.amount_msat, Some(21_000));

        let wrong_kind = provider.sign(
            1_700_000_310,
            6_002,
            vec![
                Tag::new(vec![
                    "request".into(),
                    serde_json::to_string(&request).unwrap(),
                ]),
                Tag::new(vec!["e".into(), request.id.clone()]),
                Tag::new(vec!["p".into(), customer.pubkey().to_owned()]),
            ],
            String::new(),
        );
        assert!(wrong_kind.validate_structure().is_err());
        let bad_input = customer.sign(
            1_700_000_320,
            5_001,
            vec![Tag::new(vec!["i".into(), "nope".into(), "blob".into()])],
            String::new(),
        );
        assert!(bad_input.validate_structure().is_err());

        let peer = XOnlyPublicKey::from_byte_array(
            super::decode_lower_hex::<32>(provider.pubkey(), "provider").unwrap(),
        )
        .unwrap();
        let secret = SecretKey::from_byte_array([0x90; 32]).unwrap();
        let cipher = nip04::encrypt("secret input", &secret, &peer, [9_u8; 16]).unwrap();
        let private = customer.sign(
            1_700_000_330,
            5_001,
            vec![
                Tag::new(vec!["encrypted".into()]),
                Tag::new(vec!["p".into(), provider.pubkey().to_owned()]),
            ],
            cipher.clone(),
        );
        private.validate_structure().unwrap();
        let private = open_job_request(&private).unwrap();
        assert!(private.encrypted);
        assert_eq!(
            nip04::decrypt(&cipher, &secret, &peer).unwrap(),
            "secret input"
        );
    }
}
