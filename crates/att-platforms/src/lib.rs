//! NIP-ATT client verification beyond Google Confidential Space: raw Intel
//! TDX quotes ([`tdx`]), AMD SEV-SNP reports ([`snp`]), and NVIDIA GPU
//! evidence from NRAS ([`nras`]), each checked to pinned vendor roots with
//! freshness and revocation, then matched to the release's measurements and
//! the endpoint binding. Anything unknown or unverifiable refuses.
//!
//! Parsing a report proves nothing by itself. A verified result here is
//! still not a live hardware qualification; that is recorded separately.
pub mod nras;
pub mod pki;
pub mod snp;
pub mod tdx;

#[cfg(test)]
mod tests;

use nostr::att::Level;
use serde_json::Value;

pub type Result<T> = std::result::Result<T, String>;

/// Intel's SGX root CA, as published by Intel's provisioning service.
pub const INTEL_SGX_ROOT: &[u8] = include_bytes!("../roots/intel_sgx_root_ca.der");
pub const INTEL_SGX_ROOT_SHA256: &str =
    "44a0196b2b99f889b8e149e95b807a350e7424964399e885a7cbb8ccfab674d3";
/// AMD's ARK and ASK for Milan and Genoa, from AMD's key distribution service.
pub const AMD_ARK_MILAN: &str = include_str!("../roots/amd_ark_milan.pem");
pub const AMD_ASK_MILAN: &str = include_str!("../roots/amd_ask_milan.pem");
pub const AMD_ARK_GENOA: &str = include_str!("../roots/amd_ark_genoa.pem");
pub const AMD_ASK_GENOA: &str = include_str!("../roots/amd_ask_genoa.pem");
pub const AMD_ARK_MILAN_SHA256: &str =
    "69d063b45344d26a2e94e1f4210de49ef555308287d4c174445c95639a540bcd";
pub const AMD_ARK_GENOA_SHA256: &str =
    "4c6598d19c18719c5dfd4a7d335f674e5bfe1d8f800cea2cf270c10d103db2f1";
/// NVIDIA Attestation Service GPU Intermediate 004 (valid to 2029), the
/// certificate NRAS's JWKS chains end at.
pub const NVIDIA_NRAS_INTERMEDIATE: &[u8] =
    include_bytes!("../roots/nvidia_nras_gpu_intermediate_004.der");
pub const NVIDIA_NRAS_INTERMEDIATE_SHA256: &str =
    "2df8907cf4d6c277b855d407ec178a649930a8e73f7294e8fce66e6ee27c5175";

/// Pinned trust anchors by SHA-256 of their DER bytes.
#[derive(Clone, Debug)]
pub struct Roots {
    pub intel: Vec<String>,
    pub amd: Vec<String>,
    pub nvidia: Vec<String>,
}

impl Roots {
    /// The real vendor roots shipped with this crate.
    pub fn vendor() -> Self {
        Self {
            intel: vec![INTEL_SGX_ROOT_SHA256.into()],
            amd: vec![AMD_ARK_MILAN_SHA256.into(), AMD_ARK_GENOA_SHA256.into()],
            nvidia: vec![NVIDIA_NRAS_INTERMEDIATE_SHA256.into()],
        }
    }
}

/// Collateral fetched from the vendors' services for one endpoint.
pub struct Collateral<'a> {
    pub tdx: Option<tdx::Collateral<'a>>,
    pub snp: Option<snp::Collateral<'a>>,
    /// NRAS's JWKS (`/.well-known/jwks.json`).
    pub nras_jwks: Option<&'a Value>,
}

/// What a verified raw-platform endpoint established.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verified {
    pub level: Level,
    pub platform: String,
    pub measurements: Vec<(String, String)>,
    pub gpus: Vec<String>,
}

fn decode(token: &str) -> Result<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(token)
        .map_err(|_| "raw evidence is not base64".into())
}

fn hex_bytes(text: &str) -> Result<Vec<u8>> {
    (0..text.len() / 2)
        .map(|i| {
            u8::from_str_radix(text.get(2 * i..2 * i + 2).unwrap_or("zz"), 16)
                .map_err(|_| "not hex".to_string())
        })
        .collect()
}

fn measured(release: &nostr::att::Release, register: &str, value: &str) -> Result<()> {
    let wanted = release
        .measurements
        .iter()
        .find(|m| m.register == register)
        .ok_or_else(|| format!("the release pins no {register}"))?;
    if wanted.value.to_ascii_lowercase() != value {
        return Err(format!("{register} does not match the release"));
    }
    Ok(())
}

/// Verify an endpoint whose evidence is a raw TDX quote or SEV-SNP report,
/// plus NRAS GPU evidence when the release requires a GPU.
pub fn verify(
    records: &oa_att::Records,
    policy: &oa_att::Policy,
    roots: &Roots,
    collateral: &Collateral<'_>,
    now: u64,
) -> Result<Verified> {
    let parsed = oa_att::parse(records, policy, now).map_err(|e| e.0)?;
    parsed
        .head
        .admits(&parsed.release_id, &parsed.release, now)?;
    let release = &parsed.release;
    let endpoint = &parsed.endpoint.body;
    let binding = hex_bytes(&endpoint.binding)?;
    for evidence in &endpoint.evidence {
        if !matches!(
            evidence.kind.as_str(),
            "tdx-quote" | "sev-snp-report" | "nvidia-gpu"
        ) {
            return Err(format!("unsupported evidence kind {}", evidence.kind));
        }
    }
    let cpu: Vec<_> = endpoint
        .evidence
        .iter()
        .filter(|e| e.kind != "nvidia-gpu")
        .collect();
    let [cpu] = cpu.as_slice() else {
        return Err("an endpoint carries exactly one CPU report".into());
    };
    let (platform, measurements, report_data) = match cpu.kind.as_str() {
        "tdx-quote" => {
            release
                .platform("tdx")
                .ok_or("the release is not admitted on TDX")?;
            let c = collateral.tdx.as_ref().ok_or("TDX collateral is missing")?;
            let report = tdx::verify(&decode(&cpu.token)?, c, &roots.intel, now)?;
            measured(release, "MRTD", &report.mrtd)?;
            for (i, rtmr) in report.rtmr.iter().take(3).enumerate() {
                measured(release, &format!("RTMR{i}"), rtmr)?;
            }
            let mut m = vec![("MRTD".to_string(), report.mrtd.clone())];
            m.extend(
                report
                    .rtmr
                    .iter()
                    .take(3)
                    .enumerate()
                    .map(|(i, v)| (format!("RTMR{i}"), v.clone())),
            );
            ("tdx", m, report.report_data)
        }
        _ => {
            release
                .platform("sev-snp")
                .ok_or("the release is not admitted on SEV-SNP")?;
            let c = collateral
                .snp
                .as_ref()
                .ok_or("SEV-SNP collateral is missing")?;
            let report = snp::verify(&decode(&cpu.token)?, c, &roots.amd, now)?;
            measured(release, "MEASUREMENT", &report.measurement)?;
            (
                "sev-snp",
                vec![("MEASUREMENT".to_string(), report.measurement)],
                report.report_data,
            )
        }
    };
    if report_data[..32] != binding[..] {
        return Err("the report does not carry the endpoint binding".into());
    }
    let gpu_evidence: Vec<_> = endpoint
        .evidence
        .iter()
        .filter(|e| e.kind == "nvidia-gpu")
        .collect();
    let gpus = match (&release.gpu, gpu_evidence.as_slice()) {
        (None, []) => Vec::new(),
        (None, _) => return Err("GPU evidence for a release that names no GPU".into()),
        (Some(_), []) => return Err("the release requires GPU evidence".into()),
        (Some(gpu), [evidence]) => {
            if evidence.format != "nras-jwt" {
                return Err("only NRAS GPU evidence is supported".into());
            }
            let jwks = collateral.nras_jwks.ok_or("NRAS keys are missing")?;
            nras::verify(
                &evidence.token,
                jwks,
                &roots.nvidia,
                &endpoint.binding,
                &gpu.models,
                now,
            )?
            .models
        }
        _ => return Err("an endpoint carries at most one GPU evidence".into()),
    };
    Ok(Verified {
        level: Level::Tee,
        platform: platform.into(),
        measurements,
        gpus,
    })
}
