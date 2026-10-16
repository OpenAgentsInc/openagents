//! Synthetic vendor chains from `vectors/generate.py` (pinned only here) and
//! the real vendor roots. None of this is live hardware attestation.
use crate::{Collateral, Roots, nras, pki, snp, tdx};
use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use nostr::att::{self, Level};
use nostr::domain::RelaySigner;
use p256::pkcs8::DecodePrivateKey as _;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const NOW: u64 = 1_800_000_000;
const TDX_ROOT: &[u8] = include_bytes!("../vectors/tdx/root.der");
const PCK_CHAIN: &str = include_str!("../vectors/tdx/pck_chain.pem");
const PCK_KEY: &str = include_str!("../vectors/tdx/pck_key.pem");
const TCB_INFO: &[u8] = include_bytes!("../vectors/tdx/tcb_info.json");
const TCB_INFO_STALE: &[u8] = include_bytes!("../vectors/tdx/tcb_info_stale.json");
const TCB_INFO_REVOKED: &[u8] = include_bytes!("../vectors/tdx/tcb_info_revoked.json");
const TCB_CHAIN: &str = include_str!("../vectors/tdx/tcb_signing_chain.pem");
const PCK_CRL: &[u8] = include_bytes!("../vectors/tdx/pck_crl.der");
const PCK_CRL_REVOKED: &[u8] = include_bytes!("../vectors/tdx/pck_crl_revoked.der");
const PCK_CRL_STALE: &[u8] = include_bytes!("../vectors/tdx/pck_crl_stale.der");
const ARK: &str = include_str!("../vectors/snp/ark.pem");
const ASK: &str = include_str!("../vectors/snp/ask.pem");
const VCEK: &[u8] = include_bytes!("../vectors/snp/vcek.der");
const VCEK_KEY: &str = include_str!("../vectors/snp/vcek_key.pem");
const CHIP: &str = include_str!("../vectors/snp/chip_id.hex");
const SNP_CRL: &[u8] = include_bytes!("../vectors/snp/crl.der");
const SNP_CRL_REVOKED: &[u8] = include_bytes!("../vectors/snp/crl_revoked.der");
const NV_ANCHOR: &[u8] = include_bytes!("../vectors/nras/anchor.der");
const NV_JWKS: &str = include_str!("../vectors/nras/jwks.json");
const NV_KEY: &str = include_str!("../vectors/nras/signer_key.pem");

fn roots() -> Roots {
    let ark = pki::pem_certs(ARK).unwrap().remove(0);
    Roots {
        intel: vec![pki::fingerprint(TDX_ROOT)],
        amd: vec![pki::fingerprint(&ark)],
        nvidia: vec![pki::fingerprint(NV_ANCHOR)],
    }
}

fn tdx_collateral<'a>(tcb: &'a [u8], crl: &'a [u8]) -> tdx::Collateral<'a> {
    tdx::Collateral {
        tcb_info: tcb,
        tcb_signing_chain: TCB_CHAIN,
        pck_crl: crl,
    }
}

const MRTD: [u8; 48] = [0x11; 48];
const RTMR: [[u8; 48]; 4] = [[0x21; 48], [0x22; 48], [0x23; 48], [0x24; 48]];
const MEASUREMENT: [u8; 48] = [0x31; 48];

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// A DCAP quote v4 signed by a fresh attestation key and the synthetic PCK key.
fn quote(report_data: [u8; 32], debug: bool, svn0: u8) -> Vec<u8> {
    use p256::ecdsa::{SigningKey, signature::Signer};
    let mut header = vec![0u8; 48];
    header[0..2].copy_from_slice(&4u16.to_le_bytes());
    header[2..4].copy_from_slice(&2u16.to_le_bytes());
    header[4..8].copy_from_slice(&0x81u32.to_le_bytes());
    let mut body = vec![0u8; 584];
    body[0] = svn0;
    body[120] = u8::from(debug);
    body[136..184].copy_from_slice(&MRTD);
    for (i, r) in RTMR.iter().enumerate() {
        body[328 + 48 * i..376 + 48 * i].copy_from_slice(r);
    }
    body[520..552].copy_from_slice(&report_data);
    let attest = SigningKey::from_slice(&[0x42; 32]).unwrap();
    let point = attest.verifying_key().to_encoded_point(false);
    let attest_pub = &point.as_bytes()[1..];
    let mut signed = header.clone();
    signed.extend(&body);
    let quote_sig: p256::ecdsa::Signature = attest.sign(&signed);
    let qe_auth = b"synthetic qe auth";
    let mut qe_report = vec![0u8; 384];
    let mut h = Sha256::new();
    h.update(attest_pub);
    h.update(qe_auth);
    qe_report[320..352].copy_from_slice(&h.finalize());
    let pck = SigningKey::from(p256::SecretKey::from_pkcs8_pem(PCK_KEY).unwrap());
    let qe_sig: p256::ecdsa::Signature = pck.sign(&qe_report);
    let mut cert = qe_report.clone();
    cert.extend(qe_sig.to_bytes());
    cert.extend((qe_auth.len() as u16).to_le_bytes());
    cert.extend(qe_auth);
    cert.extend(5u16.to_le_bytes());
    cert.extend((PCK_CHAIN.len() as u32).to_le_bytes());
    cert.extend(PCK_CHAIN.as_bytes());
    let mut sig = quote_sig.to_bytes().to_vec();
    sig.extend(attest_pub);
    sig.extend(6u16.to_le_bytes());
    sig.extend((cert.len() as u32).to_le_bytes());
    sig.extend(cert);
    let mut out = signed;
    out.extend((sig.len() as u32).to_le_bytes());
    out.extend(sig);
    out
}

/// An SEV-SNP report signed by the synthetic VCEK.
fn report(report_data: [u8; 32], debug: bool, tcb: [u8; 8]) -> Vec<u8> {
    use p384::ecdsa::{SigningKey, signature::Signer};
    let mut r = vec![0u8; 1184];
    r[0..4].copy_from_slice(&2u32.to_le_bytes());
    r[52..56].copy_from_slice(&1u32.to_le_bytes());
    let policy: u64 = 0x30000 | if debug { 1 << 19 } else { 0 };
    r[8..16].copy_from_slice(&policy.to_le_bytes());
    r[80..112].copy_from_slice(&report_data);
    r[144..192].copy_from_slice(&MEASUREMENT);
    r[384..392].copy_from_slice(&tcb);
    let chip: Vec<u8> = (0..64)
        .map(|i| u8::from_str_radix(&CHIP.trim()[2 * i..2 * i + 2], 16).unwrap())
        .collect();
    r[416..480].copy_from_slice(&chip);
    let key = SigningKey::from(p384::SecretKey::from_pkcs8_pem(VCEK_KEY).unwrap());
    let sig: p384::ecdsa::Signature = key.sign(&r[..0x2A0]);
    let bytes = sig.to_bytes();
    for i in 0..48 {
        r[0x2A0 + i] = bytes[47 - i];
        r[0x2A0 + 72 + i] = bytes[95 - i];
    }
    r
}

const TCB: [u8; 8] = [3, 0, 0, 0, 0, 0, 8, 115];

fn jwt(claims: &Value) -> String {
    use p384::ecdsa::{SigningKey, signature::Signer};
    let key = SigningKey::from(p384::SecretKey::from_pkcs8_pem(NV_KEY).unwrap());
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"ES384","kid":"synthetic-nras-kid"}"#);
    let body = URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).unwrap());
    let sig: p384::ecdsa::Signature = key.sign(format!("{header}.{body}").as_bytes());
    format!("{header}.{body}.{}", URL_SAFE_NO_PAD.encode(sig.to_bytes()))
}

fn nras_evidence(binding: &str, hwmodel: &str, measres: &str) -> String {
    let gpu = jwt(
        &json!({"iss":nras::ISSUER,"iat":NOW,"exp":NOW+7200,"eat_nonce":binding,"measres":measres,
        "secboot":true,"dbgstat":"disabled","hwmodel":hwmodel}),
    );
    let digest = hex(&Sha256::digest(gpu.as_bytes()));
    let overall = jwt(
        &json!({"iss":nras::ISSUER,"iat":NOW,"exp":NOW+7200,"eat_nonce":binding,
        "x-nvidia-overall-att-result":true,"submods":{"GPU-0":["DIGEST",["SHA256",digest]]}}),
    );
    serde_json::to_string(&json!([["JWT", overall], {"GPU-0": gpu}])).unwrap()
}

#[test]
fn a_tdx_quote_verifies_to_the_pinned_root_with_current_collateral() {
    let roots = roots();
    let q = quote([7; 32], false, 2);
    let report = tdx::verify(&q, &tdx_collateral(TCB_INFO, PCK_CRL), &roots.intel, NOW).unwrap();
    assert_eq!(report.mrtd, hex(&MRTD));
    assert_eq!(report.report_data[..32], [7; 32]);
    assert_eq!(report.tcb_status, "UpToDate");
    // Debug TDs, an outdated TCB level, stale or revoked collateral, a
    // revoked PCK certificate, another root, and a tampered quote all refuse.
    assert!(
        tdx::verify(
            &quote([7; 32], true, 2),
            &tdx_collateral(TCB_INFO, PCK_CRL),
            &roots.intel,
            NOW
        )
        .unwrap_err()
        .contains("debug")
    );
    assert!(
        tdx::verify(
            &quote([7; 32], false, 1),
            &tdx_collateral(TCB_INFO, PCK_CRL),
            &roots.intel,
            NOW
        )
        .unwrap_err()
        .contains("OutOfDate")
    );
    assert!(
        tdx::verify(
            &q,
            &tdx_collateral(TCB_INFO_STALE, PCK_CRL),
            &roots.intel,
            NOW
        )
        .unwrap_err()
        .contains("stale")
    );
    assert!(
        tdx::verify(
            &q,
            &tdx_collateral(TCB_INFO_REVOKED, PCK_CRL),
            &roots.intel,
            NOW
        )
        .unwrap_err()
        .contains("Revoked")
    );
    assert!(
        tdx::verify(
            &q,
            &tdx_collateral(TCB_INFO, PCK_CRL_REVOKED),
            &roots.intel,
            NOW
        )
        .unwrap_err()
        .contains("revoked")
    );
    assert!(
        tdx::verify(
            &q,
            &tdx_collateral(TCB_INFO, PCK_CRL_STALE),
            &roots.intel,
            NOW
        )
        .unwrap_err()
        .contains("stale")
    );
    assert!(
        tdx::verify(
            &q,
            &tdx_collateral(TCB_INFO, PCK_CRL),
            &Roots::vendor().intel,
            NOW
        )
        .unwrap_err()
        .contains("pinned")
    );
    let mut tampered = q.clone();
    tampered[48 + 136] ^= 1;
    assert!(
        tdx::verify(
            &tampered,
            &tdx_collateral(TCB_INFO, PCK_CRL),
            &roots.intel,
            NOW
        )
        .is_err()
    );
}

#[test]
fn an_snp_report_verifies_up_the_amd_chain_for_its_chip_and_tcb() {
    let roots = roots();
    let collateral = snp::Collateral {
        vcek: VCEK,
        ask: ASK,
        ark: ARK,
        crl: SNP_CRL,
    };
    let r = report([9; 32], false, TCB);
    let verified = snp::verify(&r, &collateral, &roots.amd, NOW).unwrap();
    assert_eq!(verified.measurement, hex(&MEASUREMENT));
    assert!(
        snp::verify(&report([9; 32], true, TCB), &collateral, &roots.amd, NOW)
            .unwrap_err()
            .contains("debug")
    );
    let mut other_tcb = TCB;
    other_tcb[6] = 9;
    assert!(
        snp::verify(
            &report([9; 32], false, other_tcb),
            &collateral,
            &roots.amd,
            NOW
        )
        .unwrap_err()
        .contains("TCB")
    );
    let revoked = snp::Collateral {
        crl: SNP_CRL_REVOKED,
        ..collateral
    };
    assert!(
        snp::verify(&r, &revoked, &roots.amd, NOW)
            .unwrap_err()
            .contains("revoked")
    );
    let collateral = snp::Collateral {
        vcek: VCEK,
        ask: ASK,
        ark: ARK,
        crl: SNP_CRL,
    };
    assert!(
        snp::verify(&r, &collateral, &Roots::vendor().amd, NOW)
            .unwrap_err()
            .contains("pinned")
    );
    let mut tampered = r.clone();
    tampered[150] ^= 1;
    assert!(
        snp::verify(&tampered, &collateral, &roots.amd, NOW)
            .unwrap_err()
            .contains("signature")
    );
}

#[test]
fn nras_gpu_evidence_is_bound_current_and_passing() {
    let roots = roots();
    let jwks: Value = serde_json::from_str(NV_JWKS).unwrap();
    let binding = "ab".repeat(32);
    let models = vec!["H100".to_string()];
    let ok = nras::verify(
        &nras_evidence(&binding, "GH100 A01 GSP BROM", "success"),
        &jwks,
        &roots.nvidia,
        &binding,
        &models,
        NOW,
    )
    .unwrap();
    assert_eq!(ok.models, ["GH100 A01 GSP BROM"]);
    assert!(
        nras::verify(
            &nras_evidence(&"cd".repeat(32), "GH100 A01 GSP BROM", "success"),
            &jwks,
            &roots.nvidia,
            &binding,
            &models,
            NOW
        )
        .is_err()
    );
    assert!(
        nras::verify(
            &nras_evidence(&binding, "GH100 A01 GSP BROM", "fail"),
            &jwks,
            &roots.nvidia,
            &binding,
            &models,
            NOW
        )
        .is_err()
    );
    assert!(
        nras::verify(
            &nras_evidence(&binding, "GB200", "success"),
            &jwks,
            &roots.nvidia,
            &binding,
            &models,
            NOW
        )
        .is_err()
    );
    assert!(
        nras::verify(
            &nras_evidence(&binding, "GH100 A01 GSP BROM", "success"),
            &jwks,
            &roots.nvidia,
            &binding,
            &models,
            NOW + 7200
        )
        .is_err()
    );
    assert!(
        nras::verify(
            &nras_evidence(&binding, "GH100 A01 GSP BROM", "success"),
            &jwks,
            &Roots::vendor().nvidia,
            &binding,
            &models,
            NOW
        )
        .is_err()
    );
}

#[test]
fn the_real_vendor_roots_are_the_published_ones() {
    assert_eq!(
        pki::fingerprint(crate::INTEL_SGX_ROOT),
        crate::INTEL_SGX_ROOT_SHA256
    );
    let ark = pki::pem_certs(crate::AMD_ARK_MILAN).unwrap().remove(0);
    assert_eq!(pki::fingerprint(&ark), crate::AMD_ARK_MILAN_SHA256);
    let genoa = pki::pem_certs(crate::AMD_ARK_GENOA).unwrap().remove(0);
    assert_eq!(pki::fingerprint(&genoa), crate::AMD_ARK_GENOA_SHA256);
    assert_eq!(
        pki::fingerprint(crate::NVIDIA_NRAS_INTERMEDIATE),
        crate::NVIDIA_NRAS_INTERMEDIATE_SHA256
    );
    // Each real root is self-signed and verifies; each real ASK chains to its ARK.
    let now = 1_800_000_000;
    pki::chain(
        &[crate::INTEL_SGX_ROOT.to_vec()],
        &[crate::INTEL_SGX_ROOT_SHA256.into()],
        now,
    )
    .unwrap();
    for (ask, ark, pin) in [
        (
            crate::AMD_ASK_MILAN,
            crate::AMD_ARK_MILAN,
            crate::AMD_ARK_MILAN_SHA256,
        ),
        (
            crate::AMD_ASK_GENOA,
            crate::AMD_ARK_GENOA,
            crate::AMD_ARK_GENOA_SHA256,
        ),
    ] {
        let mut ders = pki::pem_certs(ask).unwrap();
        ders.extend(pki::pem_certs(ark).unwrap());
        pki::chain(&ders, &[pin.into()], now).unwrap();
    }
}

/// Release, head, and endpoint events for one raw-platform endpoint.
fn records(
    platform: &str,
    gpu: bool,
    evidence: impl Fn(&str) -> Vec<att::Evidence>,
) -> (oa_att::Records, oa_att::Policy) {
    let publisher = RelaySigner::from_secret_hex(&"01".repeat(32)).unwrap();
    let endpoint_key = RelaySigner::from_secret_hex(&"02".repeat(32)).unwrap();
    let measurements = if platform == "tdx" {
        let mut m = vec![json!({"register":"MRTD","value":hex(&MRTD)})];
        m.extend((0..3).map(|i| json!({"register":format!("RTMR{i}"),"value":hex(&RTMR[i])})));
        m
    } else {
        vec![json!({"register":"MEASUREMENT","value":hex(&MEASUREMENT)})]
    };
    let release: att::Release = serde_json::from_value(json!({"v":att::RELEASE_V,"requires":[],"workload":"raw-fixture",
        "publisher":publisher.pubkey(),"image":{"reference":"example.test/raw","digest":format!("sha256:{}","aa".repeat(32))},
        "platforms":[{"kind":platform,"hwmodel":"","support":""}],"measurements":measurements,
        "gpu":if gpu {json!({"vendor":"nvidia","mode":"cc","models":["H100"]})} else {Value::Null},
        "models":[],"components":[],"source":{"repo":"https://example.test","commit":"ab".repeat(20),"recipe":"build.sh",
        "recipe_digest":format!("sha256:{}","bb".repeat(32))},"rebuilds":[],"transparency":[],"changes":"fixture","published_at":NOW-200_000})).unwrap();
    let release_event = att::release_event(&publisher, &release, NOW - 200_000).unwrap();
    let head: att::Head = serde_json::from_value(json!({"v":att::HEAD_V,"requires":[],"workload":"raw-fixture","generation":1,
        "notice_seconds":86_400,"admitted":[{"release":release_event.id,"effective_at":NOW-100_000,"retire_at":null}],"emergency":null})).unwrap();
    let head_event = att::head_event(&publisher, &head, NOW - 100_000).unwrap();
    let binding = att::binding(endpoint_key.pubkey(), None, &release_event.id).unwrap();
    let body: att::Endpoint = serde_json::from_value(json!({"v":att::ENDPOINT_V,"requires":[],"release":release_event.id,
        "endpoint":endpoint_key.pubkey(),"hpke":null,"binding":binding,"evidence":evidence(&binding),
        "operator":"03".repeat(32),"issued_at":NOW-60,"valid_until":NOW+3000})).unwrap();
    let endpoint_event = att::endpoint_event(
        &endpoint_key,
        &body,
        &"04".repeat(32),
        &head.address(publisher.pubkey()),
    )
    .unwrap();
    let policy = oa_att::Policy {
        publisher: publisher.pubkey().into(),
        workload: "raw-fixture".into(),
        required: Level::Tee,
        seen_generation: None,
    };
    (
        oa_att::Records {
            release: release_event,
            head: head_event,
            endpoint: endpoint_event,
            beacon: None,
        },
        policy,
    )
}

fn binding_bytes(binding: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&binding[2 * i..2 * i + 2], 16).unwrap();
    }
    out
}

#[test]
fn a_raw_tdx_endpoint_with_gpu_evidence_verifies_end_to_end() {
    let (records, policy) = records("tdx", true, |binding| {
        vec![
            att::Evidence {
                kind: "tdx-quote".into(),
                format: "raw".into(),
                token: STANDARD.encode(quote(binding_bytes(binding), false, 2)),
            },
            att::Evidence {
                kind: "nvidia-gpu".into(),
                format: "nras-jwt".into(),
                token: nras_evidence(binding, "GH100 A01 GSP BROM", "success"),
            },
        ]
    });
    let jwks: Value = serde_json::from_str(NV_JWKS).unwrap();
    let collateral = Collateral {
        tdx: Some(tdx_collateral(TCB_INFO, PCK_CRL)),
        snp: None,
        nras_jwks: Some(&jwks),
    };
    let verified = crate::verify(&records, &policy, &roots(), &collateral, NOW).unwrap();
    assert_eq!(verified.level, Level::Tee);
    assert_eq!(verified.platform, "tdx");
    assert_eq!(verified.gpus.len(), 1);
    // Without its GPU evidence, the same release refuses.
    let (records, policy) = records_without_gpu();
    assert!(
        crate::verify(&records, &policy, &roots(), &collateral, NOW)
            .unwrap_err()
            .contains("GPU")
    );
}

fn records_without_gpu() -> (oa_att::Records, oa_att::Policy) {
    records("tdx", true, |binding| {
        vec![att::Evidence {
            kind: "tdx-quote".into(),
            format: "raw".into(),
            token: STANDARD.encode(quote(binding_bytes(binding), false, 2)),
        }]
    })
}

#[test]
fn a_raw_snp_endpoint_checks_binding_measurement_and_unknown_claims() {
    let collateral = Collateral {
        tdx: None,
        snp: Some(snp::Collateral {
            vcek: VCEK,
            ask: ASK,
            ark: ARK,
            crl: SNP_CRL,
        }),
        nras_jwks: None,
    };
    let (good, policy) = records("sev-snp", false, |binding| {
        vec![att::Evidence {
            kind: "sev-snp-report".into(),
            format: "raw".into(),
            token: STANDARD.encode(report(binding_bytes(binding), false, TCB)),
        }]
    });
    assert_eq!(
        crate::verify(&good, &policy, &roots(), &collateral, NOW)
            .unwrap()
            .platform,
        "sev-snp"
    );
    // A report carrying some other value than the binding refuses.
    let (unbound, policy) = records("sev-snp", false, |_| {
        vec![att::Evidence {
            kind: "sev-snp-report".into(),
            format: "raw".into(),
            token: STANDARD.encode(report([0; 32], false, TCB)),
        }]
    });
    assert!(
        crate::verify(&unbound, &policy, &roots(), &collateral, NOW)
            .unwrap_err()
            .contains("binding")
    );
    // An unknown evidence kind refuses rather than being ignored.
    let (unknown, policy) = records("sev-snp", false, |binding| {
        vec![
            att::Evidence {
                kind: "sev-snp-report".into(),
                format: "raw".into(),
                token: STANDARD.encode(report(binding_bytes(binding), false, TCB)),
            },
            att::Evidence {
                kind: "mystery-tee".into(),
                format: "raw".into(),
                token: "AAAA".into(),
            },
        ]
    });
    assert!(
        crate::verify(&unknown, &policy, &roots(), &collateral, NOW)
            .unwrap_err()
            .contains("unsupported")
    );
    // GPU evidence for a release that names no GPU refuses.
    let (extra, policy) = records("sev-snp", false, |binding| {
        vec![
            att::Evidence {
                kind: "sev-snp-report".into(),
                format: "raw".into(),
                token: STANDARD.encode(report(binding_bytes(binding), false, TCB)),
            },
            att::Evidence {
                kind: "nvidia-gpu".into(),
                format: "nras-jwt".into(),
                token: nras_evidence(binding, "GH100", "success"),
            },
        ]
    });
    assert!(crate::verify(&extra, &policy, &roots(), &collateral, NOW).is_err());
}

#[test]
fn the_parse_layers_refuse_short_or_foreign_bytes() {
    assert!(snp::parse(&[0; 100]).is_err());
    assert!(snp::parse(&[0; 1184]).is_err(), "version 0 is not a report");
    assert!(tdx::parse(&[0; 10]).is_err());
    let q = quote([1; 32], false, 2);
    assert!(tdx::parse(&q[..q.len() - 10]).is_err());
    let parsed = tdx::parse(&q).unwrap();
    assert_eq!(parsed.body.len(), 584);
    assert!(parsed.pck_chain.contains("BEGIN CERTIFICATE"));
    let r = report([1; 32], false, TCB);
    assert_eq!(snp::parse(&r).unwrap().measurement, hex(&MEASUREMENT));
}

/// `ATT_FUZZ_SEEDS=<dir>` writes a synthetic report and quote as fuzz
/// seeds (test keys only) for `crates/layertree-tee-lab/fuzz`.
#[test]
fn writes_fuzz_seeds_when_asked() {
    let Some(dir) = std::env::var_os("ATT_FUZZ_SEEDS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    for (target, bytes) in [
        ("snp_report_parse", report([5; 32], false, TCB)),
        ("tdx_quote_parse", quote([5; 32], false, 2)),
    ] {
        std::fs::create_dir_all(dir.join(target)).unwrap();
        std::fs::write(dir.join(target).join("seed-synthetic"), bytes).unwrap();
    }
}
