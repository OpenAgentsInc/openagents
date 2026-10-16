//! A fake attestation report and the check a light client runs on it.
//!
//! Real SEV-SNP reports are signed with ECDSA P-384 by a VCEK chained to
//! AMD's ARK/ASK roots; TDX quotes chain to Intel's PCK roots. Neither is
//! in our lockfile, and the lab must not pretend to parse them (#11745).
//! So the "vendor" here is a secp256k1 key, and the report carries only
//! the fields the check needs:
//!
//! * `measurement`: the launch digest of the enclave image (SNP
//!   `MEASUREMENT`, 48 bytes; TDX `MRTD`/RTMRs);
//! * `report_data`: 64 bytes the guest chose, which binds the enclave key
//!   and the tree parameters.
//!
//! The release log stands in for a Rekor/Sigstore transparency log: the
//! set of measurements we published, each with the source commit a
//! reproducible build turns into it.

use bitcoin::hashes::{Hash, sha256};
use bitcoin::key::{Keypair, XOnlyPublicKey};
use bitcoin::secp256k1::{
    Message, Secp256k1, SecretKey, Signing, Verification, schnorr::Signature,
};

use crate::rules::TreeParams;
use crate::tree::tagged_hash;

pub const TAG_REPORT_DATA: &str = "LayerTreeTEE/report-data";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    SevSnp,
    Tdx,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub platform: Platform,
    pub measurement: [u8; 48],
    pub report_data: [u8; 64],
    pub signature: Signature,
}

impl Report {
    fn digest(platform: Platform, measurement: &[u8; 48], report_data: &[u8; 64]) -> [u8; 32] {
        let mut b = Vec::with_capacity(113);
        b.push(platform as u8);
        b.extend_from_slice(measurement);
        b.extend_from_slice(report_data);
        sha256::Hash::hash(&b).to_byte_array()
    }
}

/// The hardware vendor's signing key, faked.
pub struct FakeVendor {
    key: SecretKey,
}

impl FakeVendor {
    pub fn new<C: Signing>(secp: &Secp256k1<C>) -> Self {
        let (key, _) = secp.generate_keypair(&mut bitcoin::secp256k1::rand::thread_rng());
        FakeVendor { key }
    }
    pub fn root<C: Signing>(&self, secp: &Secp256k1<C>) -> XOnlyPublicKey {
        XOnlyPublicKey::from(self.key.public_key(secp))
    }
    /// What the hardware does: sign what the guest put in report_data,
    /// next to a measurement the guest cannot choose.
    pub fn sign_report<C: Signing>(
        &self,
        secp: &Secp256k1<C>,
        platform: Platform,
        measurement: [u8; 48],
        report_data: [u8; 64],
    ) -> Report {
        let d = Report::digest(platform, &measurement, &report_data);
        let sig = secp.sign_schnorr(
            &Message::from_digest(d),
            &Keypair::from_secret_key(secp, &self.key),
        );
        Report {
            platform,
            measurement,
            report_data,
            signature: sig,
        }
    }
}

/// The 64 bytes an enclave puts in `report_data`: a tagged hash of its
/// key and the tree parameters, then zeros.
pub fn report_data(params: &TreeParams) -> [u8; 64] {
    let mut b = Vec::new();
    b.extend_from_slice(&params.enclave_key.serialize());
    b.extend_from_slice(&params.operator_key);
    b.extend_from_slice(&params.tree_id);
    b.extend_from_slice(&params.depth.to_le_bytes());
    b.extend_from_slice(&params.dust_min.to_le_bytes());
    b.extend_from_slice(&params.unroll_delay.to_le_bytes());
    match params.migration_m {
        Some(m) => {
            b.push(1);
            b.extend_from_slice(&m);
        }
        None => b.push(0),
    }
    let mut out = [0u8; 64];
    out[..32].copy_from_slice(&tagged_hash(TAG_REPORT_DATA, &[&b]));
    out
}

/// One published release: a measurement and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub measurement: [u8; 48],
    pub source_commit: String,
}

/// What a light client trusts: the vendor root and our release log.
pub struct Policy {
    pub vendor_root: XOnlyPublicKey,
    pub releases: Vec<LogEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttestError {
    VendorSignature,
    UnknownMeasurement,
    ReportDataMismatch,
}

/// The check: the vendor signed it, the code is a published release, and
/// the report binds this enclave key to these tree parameters.
pub fn verify<C: Verification>(
    secp: &Secp256k1<C>,
    policy: &Policy,
    report: &Report,
    params: &TreeParams,
) -> Result<&'static str, AttestError> {
    let d = Report::digest(report.platform, &report.measurement, &report.report_data);
    secp.verify_schnorr(
        &report.signature,
        &Message::from_digest(d),
        &policy.vendor_root,
    )
    .map_err(|_| AttestError::VendorSignature)?;
    if !policy
        .releases
        .iter()
        .any(|e| e.measurement == report.measurement)
    {
        return Err(AttestError::UnknownMeasurement);
    }
    if report.report_data != report_data(params) {
        return Err(AttestError::ReportDataMismatch);
    }
    Ok("attested")
}
