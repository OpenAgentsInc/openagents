//! Verify the chosen Google-backed TDX route and its exact tree output.
//! The pure verifier accepts public evidence. `channel` adds pinned TLS.
pub mod channel;
use bitcoin::{TxOut, key::XOnlyPublicKey, secp256k1::Secp256k1};
use layertree_tee_lab::{
    attest::report_data,
    events::{Body, LogEntryJson},
    output::TreeOutput,
    rules::TreeParams,
    tree::Node,
};
use nostr::domain::Event;
use oa_att::token::Claims;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

pub const AUDIENCE: &str = "openagents.layertree.route.v1";
pub const MAX_TOKEN_BYTES: usize = 32_768;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    TooLarge,
    Token(String),
    Platform,
    Debug,
    Support,
    SecureBoot,
    Stale,
    TcbStatus,
    Revoked,
    TcbFloor,
    UnknownMeasurement,
    ReportDataMismatch,
    Release(String),
    OutputMismatch,
    Policy,
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::TooLarge => "too_large",
            Self::Token(_) => "token",
            Self::Platform => "platform",
            Self::Debug => "debug",
            Self::Support => "support",
            Self::SecureBoot => "secure_boot",
            Self::Stale => "stale",
            Self::TcbStatus => "tcb_status",
            Self::Revoked => "revoked",
            Self::TcbFloor => "tcb_floor",
            Self::UnknownMeasurement => "unknown_measurement",
            Self::ReportDataMismatch => "report_data_mismatch",
            Self::Release(_) => "release",
            Self::OutputMismatch => "output_mismatch",
            Self::Policy => "policy",
        }
    }
    pub fn event(&self, expected_image: &str) -> Body {
        Body::Attestation {
            platform: "gcp-intel-tdx".into(),
            measurement: expected_image.into(),
            log_entry: None,
            result: self.code().into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TcbFloor {
    pub evaluation_data_number: u32,
    pub attester_date_unix: i64,
    pub confidential_space_build: u64,
}

pub struct TokenPolicy<'a> {
    pub image_digest: &'a str,
    pub max_age_seconds: u64,
    pub floor: &'a TcbFloor,
}

/// Recorded mode qualifies historical captures. Live mode additionally
/// binds a caller challenge and the certificate used by its TLS connection.
pub enum Binding<'a> {
    Recorded,
    Live {
        challenge: &'a [u8; 32],
        certificate_sha256: &'a [u8; 32],
    },
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn verify_token(
    token: &str,
    params: &TreeParams,
    policy: &TokenPolicy<'_>,
    binding: &Binding<'_>,
    now: u64,
) -> Result<Claims, Error> {
    if token.len() > MAX_TOKEN_BYTES {
        return Err(Error::TooLarge);
    }
    if now.checked_add(300).is_none()
        || policy.max_age_seconds == 0
        || policy.max_age_seconds > 3600
    {
        return Err(Error::Policy);
    }
    let claims = oa_att::token::verify(token, AUDIENCE, now).map_err(|e| Error::Token(e.0))?;
    check_claims(&claims, params, policy, binding, now)?;
    Ok(claims)
}

fn check_claims(
    claims: &Claims,
    params: &TreeParams,
    policy: &TokenPolicy<'_>,
    binding: &Binding<'_>,
    now: u64,
) -> Result<(), Error> {
    if claims.hwmodel != "GCP_INTEL_TDX"
        || claims.raw["attester_tcb"] != serde_json::json!(["INTEL"])
    {
        return Err(Error::Platform);
    }
    if claims.dbgstat != "disabled-since-boot" {
        return Err(Error::Debug);
    }
    if !claims.support.iter().any(|s| s == "STABLE") {
        return Err(Error::Support);
    }
    if claims.raw["secboot"] != true {
        return Err(Error::SecureBoot);
    }
    if now < claims.iat || now.saturating_sub(claims.iat) > policy.max_age_seconds {
        return Err(Error::Stale);
    }
    let tdx = &claims.raw["tdx"];
    match tdx["gcp_attester_tcb_status"].as_str() {
        Some("UpToDate") => {}
        Some("Revoked") => return Err(Error::Revoked),
        _ => return Err(Error::TcbStatus),
    }
    let evaluation = tdx["tcb_evaluation_data_number"]
        .as_u64()
        .ok_or(Error::TcbFloor)?;
    let date = OffsetDateTime::parse(
        tdx["gcp_attester_tcb_date"]
            .as_str()
            .ok_or(Error::TcbFloor)?,
        &Rfc3339,
    )
    .map_err(|_| Error::TcbFloor)?
    .unix_timestamp();
    let build = claims
        .swversion
        .first()
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or(Error::TcbFloor)?;
    if evaluation > u64::from(u32::MAX)
        || date < 0
        || date as u64 > claims.iat
        || evaluation < u64::from(policy.floor.evaluation_data_number)
        || date < policy.floor.attester_date_unix
        || build < policy.floor.confidential_space_build
    {
        return Err(Error::TcbFloor);
    }
    if claims.image_digest != policy.image_digest {
        return Err(Error::UnknownMeasurement);
    }
    let mut nonces = vec![hex(&report_data(params)[..32])];
    if let Binding::Live {
        challenge,
        certificate_sha256,
    } = binding
    {
        nonces.push(hex(*challenge));
        nonces.push(hex(*certificate_sha256));
    }
    if claims.nonces != nonces {
        return Err(Error::ReportDataMismatch);
    }
    Ok(())
}

pub struct Evidence<'a> {
    pub token: &'a str,
    pub release: &'a Event,
    pub head: &'a Event,
    pub log_entry: &'a att_release::rekor::Entry,
    pub rebuilds: &'a [Event],
    pub consistency: Option<&'a [att_release::rekor::Hash]>,
    pub output: &'a TxOut,
    pub root: Node,
    pub unroll_key: XOnlyPublicKey,
}

pub struct Policy<'a> {
    pub params: &'a TreeParams,
    pub publisher: &'a str,
    pub workload: &'a str,
    pub min_rebuilds: usize,
    pub seen_generation: Option<u64>,
    pub max_age_seconds: u64,
    pub floor: &'a TcbFloor,
}

#[derive(Debug)]
pub struct Verified {
    pub image_digest: String,
    pub source_commit: String,
    pub release: String,
    pub issued_at: u64,
    pub tcb_evaluation_data_number: u64,
}

impl Verified {
    pub fn event(&self) -> Body {
        Body::Attestation {
            platform: "gcp-intel-tdx".into(),
            measurement: self.image_digest.clone(),
            log_entry: Some(LogEntryJson {
                measurement: self.image_digest.clone(),
                source_commit: self.source_commit.clone(),
            }),
            result: "attested".into(),
        }
    }
}

/// Compare the actual output supplied by a validated signet chain reader.
/// This pure check does not establish that caller-supplied bytes are on chain.
pub fn verify(
    evidence: &Evidence<'_>,
    policy: &Policy<'_>,
    binding: &Binding<'_>,
    witness: &mut att_release::rekor::Witness,
    now: u64,
) -> Result<Verified, Error> {
    if now.checked_add(300).is_none() {
        return Err(Error::Policy);
    }
    let key = att_release::rekor::LogKey::sigstore().map_err(Error::Release)?;
    let checked = att_release::Evidence {
        release: evidence.release,
        head: evidence.head,
        entry: Some(evidence.log_entry),
        consistency: evidence.consistency,
        rebuilds: evidence.rebuilds,
        approvals: None,
    };
    let release_policy = att_release::Policy {
        publisher: policy.publisher,
        workload: policy.workload,
        log: Some(&key),
        min_rebuilds: policy.min_rebuilds,
        seen_generation: policy.seen_generation,
    };
    // Commit witness state only when the entire attestation and output check passes.
    let mut next_witness = witness.clone();
    att_release::check_release(&checked, &release_policy, &mut next_witness, now)
        .map_err(Error::Release)?;
    let release = nostr::att::parse_release(evidence.release).map_err(Error::Release)?;
    if !release.platforms.iter().any(|p| {
        p.kind == "gcp-confidential-space" && p.hwmodel == "GCP_INTEL_TDX" && p.support == "STABLE"
    }) {
        return Err(Error::Platform);
    }
    let token_policy = TokenPolicy {
        image_digest: &release.image.digest,
        max_age_seconds: policy.max_age_seconds,
        floor: policy.floor,
    };
    let claims = verify_token(evidence.token, policy.params, &token_policy, binding, now)?;
    let script = TreeOutput {
        root_hash: evidence.root.hash,
        unroll_key: evidence.unroll_key,
    }
    .script_pubkey(
        &Secp256k1::verification_only(),
        &policy.params.enclave_key,
        policy.params.unroll_delay,
    );
    if evidence.output.value.to_sat() != evidence.root.sum
        || evidence.output.script_pubkey != script
    {
        return Err(Error::OutputMismatch);
    }
    *witness = next_witness;
    Ok(Verified {
        image_digest: release.image.digest,
        source_commit: release.source.commit,
        release: evidence.release.id.clone(),
        issued_at: claims.iat,
        tcb_evaluation_data_number: claims.raw["tdx"]["tcb_evaluation_data_number"]
            .as_u64()
            .ok_or(Error::TcbFloor)?,
    })
}
