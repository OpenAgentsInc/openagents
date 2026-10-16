//! Certificate chains, signatures, and CRLs for the vendor roots. Only the
//! signature algorithms the vendors use are accepted; anything else refuses.
use crate::Result;
use base64::Engine as _;
use rsa::{RsaPublicKey, pkcs8::DecodePublicKey, signature::Verifier};
use sha2::{Digest, Sha256, Sha384};
use x509_cert::{
    Certificate,
    crl::CertificateList,
    der::{Decode, Encode},
};

const ECDSA_SHA256: &str = "1.2.840.10045.4.3.2";
const ECDSA_SHA384: &str = "1.2.840.10045.4.3.3";
const RSA_SHA256: &str = "1.2.840.113549.1.1.11";
const RSA_SHA384: &str = "1.2.840.113549.1.1.12";
const RSA_PSS: &str = "1.2.840.113549.1.1.10";
const EC_PUBLIC_KEY: &str = "1.2.840.10045.2.1";
const P256: &str = "1.2.840.10045.3.1.7";
const P384: &str = "1.3.132.0.34";

/// Every certificate in a PEM bundle, in order, as DER.
pub fn pem_certs(text: &str) -> Result<Vec<Vec<u8>>> {
    let mut out = Vec::new();
    for block in text.split("-----BEGIN CERTIFICATE-----").skip(1) {
        let body = block
            .split("-----END CERTIFICATE-----")
            .next()
            .ok_or("malformed PEM")?;
        let clean: String = body.chars().filter(|c| !c.is_whitespace()).collect();
        out.push(
            base64::engine::general_purpose::STANDARD
                .decode(clean)
                .map_err(|_| "malformed PEM base64")?,
        );
    }
    if out.is_empty() {
        return Err("no certificate in the PEM bundle".into());
    }
    Ok(out)
}

pub fn parse(der: &[u8]) -> Result<Certificate> {
    Certificate::from_der(der).map_err(|_| "a certificate does not parse".into())
}

/// SHA-256 of DER bytes, lowercase hex.
pub fn fingerprint(der: &[u8]) -> String {
    Sha256::digest(der)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn seconds(time: &x509_cert::time::Time) -> u64 {
    time.to_unix_duration().as_secs()
}

/// Whether a certificate is valid at `now`.
pub fn current(cert: &Certificate, now: u64) -> Result<()> {
    let validity = &cert.tbs_certificate.validity;
    if now < seconds(&validity.not_before) || now >= seconds(&validity.not_after) {
        return Err("a certificate is outside its validity period".into());
    }
    Ok(())
}

/// A public key from a certificate.
pub enum Key {
    P256(p256::ecdsa::VerifyingKey),
    P384(p384::ecdsa::VerifyingKey),
    Rsa(RsaPublicKey),
}

pub fn key(cert: &Certificate) -> Result<Key> {
    let spki = &cert.tbs_certificate.subject_public_key_info;
    let bits = spki.subject_public_key.raw_bytes();
    if spki.algorithm.oid.to_string() == EC_PUBLIC_KEY {
        let curve = spki
            .algorithm
            .parameters
            .as_ref()
            .and_then(|p| p.decode_as::<x509_cert::der::asn1::ObjectIdentifier>().ok())
            .map(|o| o.to_string())
            .unwrap_or_default();
        return match curve.as_str() {
            P256 => Ok(Key::P256(
                p256::ecdsa::VerifyingKey::from_sec1_bytes(bits).map_err(|_| "bad P-256 key")?,
            )),
            P384 => Ok(Key::P384(
                p384::ecdsa::VerifyingKey::from_sec1_bytes(bits).map_err(|_| "bad P-384 key")?,
            )),
            _ => Err("unsupported elliptic curve".into()),
        };
    }
    let der = spki.to_der().map_err(|_| "bad key encoding")?;
    Ok(Key::Rsa(
        RsaPublicKey::from_public_key_der(&der).map_err(|_| "unsupported public key")?,
    ))
}

/// Verify `signature` over `message` under the named algorithm.
pub fn verify(key: &Key, algorithm: &str, message: &[u8], signature: &[u8]) -> Result<()> {
    let ok = match (algorithm, key) {
        (ECDSA_SHA256, Key::P256(k)) => {
            p256::ecdsa::Signature::from_der(signature).is_ok_and(|s| k.verify(message, &s).is_ok())
        }
        (ECDSA_SHA384, Key::P384(k)) => {
            p384::ecdsa::Signature::from_der(signature).is_ok_and(|s| k.verify(message, &s).is_ok())
        }
        (RSA_SHA256, Key::Rsa(k)) => rsa::pkcs1v15::Signature::try_from(signature).is_ok_and(|s| {
            rsa::pkcs1v15::VerifyingKey::<Sha256>::new(k.clone())
                .verify(message, &s)
                .is_ok()
        }),
        (RSA_SHA384, Key::Rsa(k)) => rsa::pkcs1v15::Signature::try_from(signature).is_ok_and(|s| {
            rsa::pkcs1v15::VerifyingKey::<Sha384>::new(k.clone())
                .verify(message, &s)
                .is_ok()
        }),
        // AMD signs with RSASSA-PSS, SHA-384, MGF1-SHA-384, salt 48.
        (RSA_PSS, Key::Rsa(k)) => rsa::pss::Signature::try_from(signature).is_ok_and(|s| {
            rsa::pss::VerifyingKey::<Sha384>::new(k.clone())
                .verify(message, &s)
                .is_ok()
        }),
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err("a signature does not verify".into())
    }
}

/// Verify that `child` is signed by `issuer`.
pub fn signed_by(child: &Certificate, issuer: &Certificate) -> Result<()> {
    if child.tbs_certificate.issuer != issuer.tbs_certificate.subject {
        return Err("a certificate names another issuer".into());
    }
    let tbs = child
        .tbs_certificate
        .to_der()
        .map_err(|_| "bad certificate encoding")?;
    verify(
        &key(issuer)?,
        &child.signature_algorithm.oid.to_string(),
        &tbs,
        child.signature.raw_bytes(),
    )
}

/// Verify a chain `[leaf, …, root]` whose root DER hashes to one of
/// `anchors`, each certificate current at `now`. Returns the parsed chain.
pub fn chain(ders: &[Vec<u8>], anchors: &[String], now: u64) -> Result<Vec<Certificate>> {
    let root = ders.last().ok_or("an empty certificate chain")?;
    if !anchors.contains(&fingerprint(root)) {
        return Err("the chain does not end at a pinned vendor root".into());
    }
    let certs: Vec<Certificate> = ders.iter().map(|d| parse(d)).collect::<Result<_>>()?;
    for cert in &certs {
        current(cert, now)?;
    }
    for pair in certs.windows(2) {
        signed_by(&pair[0], &pair[1])?;
    }
    let last = certs.last().ok_or("chain")?;
    signed_by(last, last)?;
    Ok(certs)
}

/// Check a CRL signed by `issuer`, current at `now`, and refuse if it lists
/// any of `serials`.
pub fn crl(der: &[u8], issuer: &Certificate, serials: &[&Certificate], now: u64) -> Result<()> {
    let list = CertificateList::from_der(der).map_err(|_| "the revocation list does not parse")?;
    if list.tbs_cert_list.issuer != issuer.tbs_certificate.subject {
        return Err("the revocation list is from another issuer".into());
    }
    let tbs = list
        .tbs_cert_list
        .to_der()
        .map_err(|_| "bad revocation list")?;
    verify(
        &key(issuer)?,
        &list.signature_algorithm.oid.to_string(),
        &tbs,
        list.signature.raw_bytes(),
    )?;
    let next = list
        .tbs_cert_list
        .next_update
        .as_ref()
        .ok_or("the revocation list has no next update")?;
    if now >= seconds(next) {
        return Err("the revocation list is stale".into());
    }
    for revoked in list.tbs_cert_list.revoked_certificates.iter().flatten() {
        if serials
            .iter()
            .any(|c| c.tbs_certificate.serial_number == revoked.serial_number)
        {
            return Err("a certificate in the chain is revoked".into());
        }
    }
    Ok(())
}

/// A certificate extension's raw value by OID.
pub fn extension<'a>(cert: &'a Certificate, oid: &str) -> Option<&'a [u8]> {
    cert.tbs_certificate
        .extensions
        .as_ref()?
        .iter()
        .find(|e| e.extn_id.to_string() == oid)
        .map(|e| e.extn_value.as_bytes())
}

/// A minimal DER reader for vendor extensions: `(tag, body, rest)`.
pub fn tlv(input: &[u8]) -> Result<(u8, &[u8], &[u8])> {
    let (&tag, rest) = input.split_first().ok_or("truncated DER")?;
    let (&first, rest) = rest.split_first().ok_or("truncated DER")?;
    let (len, rest) = if first < 0x80 {
        (usize::from(first), rest)
    } else {
        let count = usize::from(first & 0x7f);
        if count == 0 || count > 4 || rest.len() < count {
            return Err("bad DER length".into());
        }
        let len = rest[..count]
            .iter()
            .fold(0usize, |acc, b| (acc << 8) | usize::from(*b));
        (len, &rest[count..])
    };
    if rest.len() < len {
        return Err("truncated DER".into());
    }
    Ok((tag, &rest[..len], &rest[len..]))
}
