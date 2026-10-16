//! Raw AMD SEV-SNP evidence: an attestation report signed by the chip's VCEK,
//! checked up AMD's ARK/ASK/VCEK chain, the AMD revocation list, and the
//! VCEK's own chip ID and TCB extensions.
use crate::{Result, pki};

const REPORT: usize = 1184;
const SIGNED: usize = 0x2A0;
const AMD: &str = "1.3.6.1.4.1.3704.1.";

/// What AMD's key distribution service returned for one chip.
pub struct Collateral<'a> {
    /// The VCEK certificate (DER) for this chip and reported TCB.
    pub vcek: &'a [u8],
    /// The ASK (PEM) for the product line.
    pub ask: &'a str,
    /// The ARK (PEM) for the product line.
    pub ark: &'a str,
    /// AMD's revocation list for the product line (DER), signed by the ARK.
    pub crl: &'a [u8],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub measurement: String,
    pub report_data: [u8; 64],
    pub chip_id: String,
    pub reported_tcb: [u8; 8],
    pub policy: u64,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn small_integer(cert: &x509_cert::Certificate, oid: &str) -> Result<u8> {
    let value =
        pki::extension(cert, &format!("{AMD}{oid}")).ok_or("the VCEK lacks a TCB extension")?;
    let (tag, body, _) = pki::tlv(value)?;
    let digits: &[u8] = match body {
        [0, rest @ ..] => rest,
        other => other,
    };
    if tag != 0x02 || digits.len() > 1 {
        return Err("a VCEK TCB extension is not a small integer".into());
    }
    Ok(digits.first().copied().unwrap_or(0))
}

/// Verify an SEV-SNP report. `arks` are pinned SHA-256 fingerprints of the
/// product lines' ARKs (or a synthetic ARK in fixtures).
pub fn verify(
    report: &[u8],
    collateral: &Collateral<'_>,
    arks: &[String],
    now: u64,
) -> Result<Report> {
    if report.len() != REPORT {
        return Err("an SEV-SNP report is 1184 bytes".into());
    }
    let version = u32::from_le_bytes(report[0..4].try_into().map_err(|_| "version")?);
    if !(2..=5).contains(&version) {
        return Err("an unsupported SEV-SNP report version".into());
    }
    if u32::from_le_bytes(report[52..56].try_into().map_err(|_| "algo")?) != 1 {
        return Err("the report is not signed with ECDSA P-384".into());
    }
    let policy = u64::from_le_bytes(report[8..16].try_into().map_err(|_| "policy")?);
    if policy & (1 << 19) != 0 {
        return Err("the guest policy allows debugging".into());
    }
    // The chain VCEK → ASK → ARK, the ARK pinned, nothing revoked.
    let mut ders = vec![collateral.vcek.to_vec()];
    ders.extend(pki::pem_certs(collateral.ask)?);
    ders.extend(pki::pem_certs(collateral.ark)?);
    let chain = pki::chain(&ders, arks, now)?;
    let ark = chain.last().ok_or("chain")?;
    pki::crl(collateral.crl, ark, &chain.iter().collect::<Vec<_>>(), now)?;
    // The VCEK is this chip's, at this TCB.
    let vcek = &chain[0];
    let chip_id = &report[416..480];
    let hwid = pki::extension(vcek, &format!("{AMD}4")).ok_or("the VCEK names no chip")?;
    let (tag, hwid, _) = pki::tlv(hwid)?;
    if tag != 0x04 || hwid != chip_id {
        return Err("the VCEK belongs to another chip".into());
    }
    let mut reported_tcb = [0u8; 8];
    reported_tcb.copy_from_slice(&report[384..392]);
    let expected = [
        (reported_tcb[0], "3.1"),
        (reported_tcb[1], "3.2"),
        (reported_tcb[6], "3.3"),
        (reported_tcb[7], "3.8"),
    ];
    for (have, oid) in expected {
        if small_integer(vcek, oid)? != have {
            return Err("the VCEK was issued for another TCB".into());
        }
    }
    // The signature: r and s are 72-byte little-endian fields.
    let pki::Key::P384(key) = pki::key(vcek)? else {
        return Err("the VCEK key is not P-384".into());
    };
    let to_be = |range: std::ops::Range<usize>| -> Result<[u8; 48]> {
        let field = &report[range];
        if field[48..].iter().any(|b| *b != 0) {
            return Err("a signature component is too large".into());
        }
        let mut be = [0u8; 48];
        for (i, b) in field[..48].iter().enumerate() {
            be[47 - i] = *b;
        }
        Ok(be)
    };
    let (r, s) = (
        to_be(SIGNED..SIGNED + 72)?,
        to_be(SIGNED + 72..SIGNED + 144)?,
    );
    let sig =
        p384::ecdsa::Signature::from_scalars(r, s).map_err(|_| "malformed report signature")?;
    use p384::ecdsa::signature::Verifier;
    key.verify(&report[..SIGNED], &sig)
        .map_err(|_| "the report signature does not verify")?;
    let mut report_data = [0u8; 64];
    report_data.copy_from_slice(&report[80..144]);
    Ok(Report {
        measurement: hex(&report[144..192]),
        report_data,
        chip_id: hex(chip_id),
        reported_tcb,
        policy,
    })
}
