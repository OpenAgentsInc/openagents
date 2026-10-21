//! The original-weight CUDA Flint service (#12425) and the MLX INT4 Metal
//! service on Apple silicon (#12422).
//!
//! Systemd owns the native worker and Pylon separately. The worker keeps its
//! generation lock after a caller disconnects; Pylon stays unavailable while
//! that worker is busy or does not report the pinned checkpoint and profile.
//!
//! Both profiles are routed as the model `flint`. Each advertises its own
//! artifact hash and numerical profile, in the beacon's model string and its
//! `native_model` metadata, so a buyer can tell them apart or pin one.

use std::sync::Arc;
use std::time::Duration;

use nostr::pylon::Beacon;
use serde_json::{Value, json};

use crate::engine::{Engine, Generation, Pending, Psionic, Sampling, Turn};
use crate::provider::Config;

pub const MODEL: &str = "flint";
pub const REVISION: &str = "2a247be0c0aec895ccb0ac7ca92229ec6df37288";
/// SHA-256 of the release manifest, which binds every checkpoint file's hash.
pub const ARTIFACT: &str =
    "sha256:e2e253557466548339ec3ba3d866ef6cd60440dfc895de7717b81f3ba320f58f";
pub const PROFILE: &str = "bf16_weights_f32_compute_v1";
pub const CONTEXT: u64 = 8192;
pub const OUTPUT: u32 = 6144;
pub const DEADLINE: Duration = Duration::from_secs(900);
pub const INPUT_BYTES: usize = 8192;

/// One pinned native Flint worker: its checkpoint identity and served limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
    /// The `--text-profile` name.
    pub name: &'static str,
    /// The model ID the worker lists at `/v1/models`.
    pub served_id: &'static str,
    pub revision: &'static str,
    pub artifact: &'static str,
    pub numerical_profile: &'static str,
    pub backend: &'static str,
    pub context: u64,
    pub output: u32,
    pub deadline: Duration,
    pub input_bytes: usize,
    /// The memory class the beacon reports by default.
    pub vram_gb: u64,
}

/// The original BF16 checkpoint on one CUDA GPU (#12419, #12425).
pub const CUDA: Profile = Profile {
    name: MODEL,
    served_id: "RecursiveSI/flint",
    revision: REVISION,
    artifact: ARTIFACT,
    numerical_profile: PROFILE,
    backend: "cuda",
    context: CONTEXT,
    output: OUTPUT,
    deadline: DEADLINE,
    input_bytes: INPUT_BYTES,
    vram_gb: 96,
};

/// Recursive's MLX INT4 checkpoint (`RecursiveSI/flint-int4`) on Apple
/// silicon through Psionic's Metal kernels (#12422). The artifact is the
/// SHA-256 of `psionic-flint/testdata/mlx-int4-artifact.json`, which binds
/// every shard's digest. 32,768 tokens of context hold a 16,384-token
/// answer, enough for Flint's 4.3K-token median reasoning and its long tail.
pub const METAL: Profile = Profile {
    name: "flint-metal",
    served_id: "RecursiveSI/flint-int4",
    revision: "3b93f9053b530488405ae9fca51d4900d6ce21f0",
    artifact: "sha256:83be6d422b14b0e19070133ac4bfcffdfcd4265847305a68503193e06a3cd8da",
    numerical_profile: "mlx_int4_metal_v1",
    backend: "metal",
    context: 32_768,
    output: 16_384,
    deadline: Duration::from_secs(900),
    input_bytes: 65_536,
    vram_gb: 128,
};

/// The profile a `--text-profile` name selects.
#[must_use]
pub fn profile(name: &str) -> Option<&'static Profile> {
    [&CUDA, &METAL].into_iter().find(|p| p.name == name)
}

impl Profile {
    /// The beacon's model string: the routed family and this artifact.
    #[must_use]
    pub fn advertised(&self) -> String {
        format!("{MODEL}@{}", self.artifact)
    }
}

#[must_use]
pub fn advertised() -> String {
    CUDA.advertised()
}

/// Configure only the tested original-weight CUDA profile.
pub fn configure(config: &mut Config) {
    configure_profile(config, &CUDA);
}

/// Configure a pinned profile: one slot, its answer allowance and deadline.
pub fn configure_profile(config: &mut Config, profile: &Profile) {
    config.slots = 1;
    config.max_tokens = profile.output;
    config.job_timeout = profile.deadline;
    config.text_profile = Some(profile.name.into());
}

/// Attach the complete model's declared limits without making it a model group.
pub(crate) fn describe(beacon: &mut Beacon, profile: &Profile) {
    let meta = beacon.meta.get_or_insert_with(|| json!({}));
    meta[crate::job::INPUT_LIMIT] = json!(profile.input_bytes);
    meta["deadline_secs"] = json!(profile.deadline.as_secs());
    meta["max_output_tokens"] = json!(profile.output);
    meta["native_model"] = json!({"revision":profile.revision,"artifact":profile.artifact,
        "numerical_profile":profile.numerical_profile});
    meta["request_limits"] = json!({"max_context_tokens":profile.context,
        "max_input_tokens":profile.context-u64::from(profile.output),
        "max_input_bytes":profile.input_bytes,"modes":["thinking"]});
    meta["sampling"] = json!({"temperature":1.0,"top_p":0.95,"top_k":20,"seed":42});
}

/// The advertised deadline for a complete model or a model-group head.
#[must_use]
pub fn deadline(beacon: &Beacon) -> Option<u64> {
    beacon
        .group_head()
        .and_then(|g| g.deadline_secs)
        .or_else(|| {
            beacon
                .meta
                .as_ref()?
                .get("deadline_secs")?
                .as_u64()
                .filter(|n| (1..=nostr::pylon::deadline::MAX_SECS).contains(n))
        })
}

fn qualified(body: &Value, own_generation: bool, profile: &Profile) -> bool {
    let info = &body["psionic"];
    body["data"]
        .as_array()
        .is_some_and(|models| models.len() == 1 && models[0]["id"] == profile.served_id)
        && info["revision"] == profile.revision
        && info["artifact"] == profile.artifact
        && info["numerical_profile"] == profile.numerical_profile
        && info["backend"] == profile.backend
        && info["max_context_tokens"]
            .as_u64()
            .is_some_and(|n| n >= profile.context)
        && (info["busy"] == false || (own_generation && info["busy"] == true))
}

pub struct Native {
    profile: &'static Profile,
    inner: Psionic,
    models: String,
    client: reqwest::Client,
    active: std::sync::atomic::AtomicBool,
}

struct Active<'a>(&'a std::sync::atomic::AtomicBool);
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
    }
}

impl Native {
    /// Connect to a loopback native worker; never expose its HTTP endpoint publicly.
    pub fn connect(base: &str) -> Result<Arc<dyn Engine>, String> {
        Self::connect_profile(base, &CUDA)
    }

    /// Connect to a loopback worker that must report `profile`'s checkpoint.
    pub fn connect_profile(
        base: &str,
        profile: &'static Profile,
    ) -> Result<Arc<dyn Engine>, String> {
        let url = reqwest::Url::parse(base).map_err(|e| e.to_string())?;
        if url.scheme() != "http"
            || url.host_str() != Some("127.0.0.1")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err("Flint needs --engine http://127.0.0.1:PORT".into());
        }
        Ok(Arc::new(Self {
            profile,
            inner: Psionic::new(base, &profile.advertised())?
                .with_sampling(Sampling::new(1.0, 0.95, 42)?)
                .with_top_k(20)
                .with_request_timeout(profile.deadline + Duration::from_secs(10)),
            models: format!("{}/v1/models", base.trim_end_matches('/')),
            active: std::sync::atomic::AtomicBool::new(false),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .map_err(|e| e.to_string())?,
        }))
    }
}

impl Engine for Native {
    fn model(&self) -> &str {
        self.inner.model()
    }
    fn recovery_required(&self) -> bool {
        self.inner.recovery_required()
    }
    fn healthy(&self) -> Pending<'_, bool> {
        Box::pin(async {
            let Ok(response) = self.client.get(&self.models).send().await else {
                return false;
            };
            response.status().is_success()
                && response.json::<Value>().await.is_ok_and(|body| {
                    qualified(
                        &body,
                        self.active.load(std::sync::atomic::Ordering::Acquire),
                        self.profile,
                    )
                })
        })
    }
    fn generate<'a>(
        &'a self,
        turns: &'a [Turn],
        max_tokens: u32,
    ) -> Pending<'a, Result<Generation, String>> {
        Box::pin(async move {
            if max_tokens != self.profile.output {
                return Err("Flint answer allowance differs from its served profile".into());
            }
            if !self.healthy().await {
                return Err("Flint is busy or restarting. Try again shortly.".into());
            }
            if self
                .active
                .compare_exchange(
                    false,
                    true,
                    std::sync::atomic::Ordering::AcqRel,
                    std::sync::atomic::Ordering::Acquire,
                )
                .is_err()
            {
                return Err("Flint is busy. Try again shortly.".into());
            }
            let active = Active(&self.active);
            let result = self.inner.generate(turns, max_tokens).await;
            drop(active);
            if self.inner.recovery_required() {
                self.settled().await;
            }
            result
        })
    }

    fn settled(&self) -> Pending<'_, ()> {
        Box::pin(async move {
            while !self.healthy().await {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_requires_the_exact_profile_and_an_idle_worker() {
        let body = json!({"data":[{"id":"RecursiveSI/flint"}],"psionic":{
            "revision":REVISION,"artifact":ARTIFACT,"numerical_profile":PROFILE,
            "max_context_tokens":CONTEXT,"busy":false,"backend":"cuda"}});
        assert!(qualified(&body, false, &CUDA));
        let mut busy = body.clone();
        busy["psionic"]["busy"] = json!(true);
        assert!(!qualified(&busy, false, &CUDA));
        assert!(qualified(&busy, true, &CUDA));
        for (key, wrong) in [
            ("revision", json!("other")),
            ("artifact", json!("other")),
            ("numerical_profile", json!("bf16")),
            ("max_context_tokens", json!(4096)),
            ("busy", json!(true)),
            ("backend", json!("cpu")),
        ] {
            let mut changed = body.clone();
            changed["psionic"][key] = wrong;
            assert!(!qualified(&changed, false, &CUDA), "{key}");
        }
        assert!(Native::connect("http://0.0.0.0:8080").is_err());
    }

    #[test]
    fn each_profile_accepts_only_its_own_worker() {
        let metal = json!({"data":[{"id":"RecursiveSI/flint-int4"}],"psionic":{
            "revision":METAL.revision,"artifact":METAL.artifact,"numerical_profile":METAL.numerical_profile,
            "max_context_tokens":METAL.context,"busy":false,"backend":"metal"}});
        assert!(qualified(&metal, false, &METAL));
        assert!(!qualified(&metal, false, &CUDA));
        let mut claims_bf16 = metal.clone();
        claims_bf16["psionic"]["artifact"] = json!(ARTIFACT);
        assert!(!qualified(&claims_bf16, false, &METAL));
        assert_ne!(METAL.advertised(), CUDA.advertised());
        assert!(METAL.advertised().starts_with("flint@sha256:"));
        assert_eq!(profile("flint"), Some(&CUDA));
        assert_eq!(profile("flint-metal"), Some(&METAL));
        assert_eq!(profile("other"), None);
    }
}
