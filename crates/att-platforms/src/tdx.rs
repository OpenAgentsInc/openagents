//! Raw Intel TDX evidence: a DCAP quote v4 (ECDSA-256 attestation key) with
//! QE report certification data and a PCK certificate chain, checked against
//! Intel's SGX root, the PCK revocation list, and signed TCB Info collateral.
use crate::{Result, pki};
use serde_json::Value;
use sha2::{Digest, Sha256};

const HEADER: usize = 48;
const BODY: usize = 584;
const QE_REPORT: usize = 384;
const TEE_TDX: u32 = 0x81;
const FMSPC_OID: &str = "1.2.840.113741.1.13.1.4";
const SGX_EXTENSIONS: &str = "1.2.840.113741.1.13.1";

/// Collateral fetched from Intel's provisioning service (PCS) for one quote.
pub struct Collateral<'a> {
    /// `{"tcbInfo":{…},"signature":"<hex r||s>"}`, exact bytes.
    pub tcb_info: &'a [u8],
    /// The TCB signing certificate chain (PEM), ending at the root.
    pub tcb_signing_chain: &'a str,
    /// The PCK revocation list (DER), signed by the PCK platform/processor CA.
    pub pck_crl: &'a [u8],
}

/// What a verified quote reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub mrtd: String,
    pub rtmr: [String; 4],
    pub report_data: [u8; 64],
    pub tee_tcb_svn: [u8; 16],
    pub fmspc: String,
    pub tcb_status: String,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn u16le(b: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        b.get(at..at + 2)
            .ok_or("truncated quote")?
            .try_into()
            .map_err(|_| "u16")?,
    ))
}
fn u32le(b: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        b.get(at..at + 4)
            .ok_or("truncated quote")?
            .try_into()
            .map_err(|_| "u32")?,
    ))
}
fn take<'a>(b: &'a [u8], at: usize, len: usize) -> Result<&'a [u8]> {
    b.get(at..at + len).ok_or_else(|| "truncated quote".into())
}

/// Seconds since the epoch for an RFC 3339 UTC time like `2099-01-01T00:00:00Z`.
pub fn rfc3339(text: &str) -> Result<u64> {
    let b = text.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return Err("a collateral time is not RFC 3339 UTC".into());
    }
    let num = |r: std::ops::Range<usize>| -> Result<i64> {
        text.get(r)
            .and_then(|s| s.parse::<i64>().ok())
            .ok_or_else(|| "bad time".into())
    };
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hh, mm, ss) = (num(11..13)?, num(14..16)?, num(17..19)?);
    // Days from civil (Howard Hinnant).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hh * 3600 + mm * 60 + ss).map_err(|_| "time before 1970".into())
}

fn ecdsa_raw(key: &p256::ecdsa::VerifyingKey, message: &[u8], raw: &[u8]) -> Result<()> {
    use p256::ecdsa::signature::Verifier;
    let sig = p256::ecdsa::Signature::from_slice(raw).map_err(|_| "malformed raw signature")?;
    key.verify(message, &sig)
        .map_err(|_| "a quote signature does not verify".into())
}

fn fmspc_of(cert: &x509_cert::Certificate) -> Result<String> {
    let value =
        pki::extension(cert, SGX_EXTENSIONS).ok_or("the PCK certificate has no SGX extensions")?;
    let (_, mut seq, _) = pki::tlv(value)?;
    let fmspc_oid = {
        // DER of the FMSPC OID, compared as bytes.
        let oid = x509_cert::der::asn1::ObjectIdentifier::new_unwrap(FMSPC_OID);
        oid.as_bytes().to_vec()
    };
    while !seq.is_empty() {
        let (_, item, rest) = pki::tlv(seq)?;
        let (tag, oid, value) = pki::tlv(item)?;
        if tag == 0x06 && oid == fmspc_oid.as_slice() {
            let (tag, fmspc, _) = pki::tlv(value)?;
            if tag == 0x04 && fmspc.len() == 6 {
                return Ok(hex(fmspc));
            }
        }
        seq = rest;
    }
    Err("the PCK certificate names no FMSPC".into())
}

/// Verify a TDX quote end to end. `roots` are pinned SHA-256 fingerprints of
/// Intel's SGX root (or a synthetic root in fixtures).
pub fn verify(
    quote: &[u8],
    collateral: &Collateral<'_>,
    roots: &[String],
    now: u64,
) -> Result<Report> {
    if u16le(quote, 0)? != 4 || u16le(quote, 2)? != 2 || u32le(quote, 4)? != TEE_TDX {
        return Err("not a TDX quote v4 with an ECDSA-256 attestation key".into());
    }
    let signed = take(quote, 0, HEADER + BODY)?;
    let body = take(quote, HEADER, BODY)?;
    let sig_len = u32le(quote, HEADER + BODY)? as usize;
    let sig = take(quote, HEADER + BODY + 4, sig_len)?;
    let quote_sig = take(sig, 0, 64)?;
    let attest_pub = take(sig, 64, 64)?;
    if u16le(sig, 128)? != 6 {
        return Err("the quote does not carry QE report certification data".into());
    }
    let cert_len = u32le(sig, 130)? as usize;
    let cert = take(sig, 134, cert_len)?;
    let qe_report = take(cert, 0, QE_REPORT)?;
    let qe_sig = take(cert, QE_REPORT, 64)?;
    let auth_len = u16le(cert, QE_REPORT + 64)? as usize;
    let qe_auth = take(cert, QE_REPORT + 66, auth_len)?;
    let at = QE_REPORT + 66 + auth_len;
    if u16le(cert, at)? != 5 {
        return Err("the quote does not carry a PCK certificate chain".into());
    }
    let pem_len = u32le(cert, at + 2)? as usize;
    let pem = std::str::from_utf8(take(cert, at + 6, pem_len)?)
        .map_err(|_| "the PCK chain is not PEM")?;
    let pem = pem.trim_end_matches('\0');
    // 1. The PCK chain ends at a pinned root and is not revoked.
    let chain = pki::chain(&pki::pem_certs(pem)?, roots, now)?;
    let issuer = chain.get(1).ok_or("the PCK chain has no issuing CA")?;
    pki::crl(
        collateral.pck_crl,
        issuer,
        &chain.iter().collect::<Vec<_>>(),
        now,
    )?;
    // 2. The QE report is signed by the PCK key and binds the attestation key.
    let pki::Key::P256(pck_key) = pki::key(&chain[0])? else {
        return Err("the PCK key is not P-256".into());
    };
    ecdsa_raw(&pck_key, qe_report, qe_sig)?;
    let mut bound = Sha256::new();
    bound.update(attest_pub);
    bound.update(qe_auth);
    let expected: [u8; 32] = bound.finalize().into();
    if qe_report[320..352] != expected || qe_report[352..384].iter().any(|b| *b != 0) {
        return Err("the QE report does not bind the attestation key".into());
    }
    // 3. The quote is signed by the attestation key.
    let mut sec1 = vec![0x04];
    sec1.extend_from_slice(attest_pub);
    let attest =
        p256::ecdsa::VerifyingKey::from_sec1_bytes(&sec1).map_err(|_| "bad attestation key")?;
    ecdsa_raw(&attest, signed, quote_sig)?;
    // 4. Debug TDs are refused.
    if body[120] & 1 != 0 {
        return Err("the TD runs in debug mode".into());
    }
    let mut tee_tcb_svn = [0u8; 16];
    tee_tcb_svn.copy_from_slice(&body[0..16]);
    // 5. Signed, current TCB Info for this platform; the quote's TCB level is up to date.
    let fmspc = fmspc_of(&chain[0])?;
    let tcb_status = tcb_status(collateral, roots, &fmspc, &tee_tcb_svn, now)?;
    let mut report_data = [0u8; 64];
    report_data.copy_from_slice(&body[520..584]);
    Ok(Report {
        mrtd: hex(&body[136..184]),
        rtmr: [
            hex(&body[328..376]),
            hex(&body[376..424]),
            hex(&body[424..472]),
            hex(&body[472..520]),
        ],
        report_data,
        tee_tcb_svn,
        fmspc,
        tcb_status,
    })
}

fn tcb_status(
    collateral: &Collateral<'_>,
    roots: &[String],
    fmspc: &str,
    svn: &[u8; 16],
    now: u64,
) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct Signed<'a> {
        #[serde(borrow, rename = "tcbInfo")]
        tcb_info: &'a serde_json::value::RawValue,
        signature: String,
    }
    let text = std::str::from_utf8(collateral.tcb_info).map_err(|_| "TCB Info is not UTF-8")?;
    let signed: Signed<'_> = serde_json::from_str(text).map_err(|_| "TCB Info does not parse")?;
    let chain = pki::chain(&pki::pem_certs(collateral.tcb_signing_chain)?, roots, now)?;
    let pki::Key::P256(signer) = pki::key(&chain[0])? else {
        return Err("the TCB signing key is not P-256".into());
    };
    let raw: Vec<u8> = (0..signed.signature.len() / 2)
        .map(|i| u8::from_str_radix(signed.signature.get(2 * i..2 * i + 2).unwrap_or("zz"), 16))
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| "TCB Info signature is not hex")?;
    ecdsa_raw(&signer, signed.tcb_info.get().as_bytes(), &raw)?;
    let info: Value = serde_json::from_str(signed.tcb_info.get()).map_err(|_| "TCB Info body")?;
    if info["id"] != "TDX" || info["fmspc"] != fmspc {
        return Err("the TCB Info is for another platform".into());
    }
    if rfc3339(info["nextUpdate"].as_str().unwrap_or_default())? <= now {
        return Err("the TCB Info collateral is stale".into());
    }
    for level in info["tcbLevels"].as_array().into_iter().flatten() {
        let components = level["tcb"]["tdxtcbcomponents"]
            .as_array()
            .ok_or("TCB level components")?;
        if components.len() != 16 {
            return Err("a TCB level does not have 16 components".into());
        }
        let applies = components.iter().zip(svn.iter()).all(|(c, have)| {
            c["svn"]
                .as_u64()
                .is_some_and(|need| u64::from(*have) >= need)
        });
        if applies {
            let status = level["tcbStatus"].as_str().unwrap_or_default().to_string();
            if status != "UpToDate" {
                return Err(format!("the platform TCB is {status}"));
            }
            return Ok(status);
        }
    }
    Err("no TCB level applies to this quote".into())
}
