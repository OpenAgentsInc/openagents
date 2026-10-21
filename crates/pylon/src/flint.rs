//! The original-weight CUDA Flint service (#12425).
//!
//! Systemd owns the native worker and Pylon separately. The worker keeps its
//! generation lock after a caller disconnects; Pylon stays unavailable while
//! that worker is busy or does not report the pinned checkpoint and profile.

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

#[must_use]
pub fn advertised() -> String {
    format!("{MODEL}@{ARTIFACT}")
}

/// Configure only the tested original-weight CUDA profile.
pub fn configure(config: &mut Config) {
    config.slots = 1;
    config.max_tokens = OUTPUT;
    config.job_timeout = DEADLINE;
    config.text_profile = Some(MODEL.into());
}

/// Attach the complete model's declared limits without making it a model group.
pub(crate) fn describe(beacon: &mut Beacon) {
    let meta = beacon.meta.get_or_insert_with(|| json!({}));
    meta[crate::job::INPUT_LIMIT] = json!(INPUT_BYTES);
    meta["deadline_secs"] = json!(DEADLINE.as_secs());
    meta["max_output_tokens"] = json!(OUTPUT);
    meta["native_model"] =
        json!({"revision":REVISION,"artifact":ARTIFACT,"numerical_profile":PROFILE});
    meta["request_limits"] = json!({"max_context_tokens":CONTEXT,"max_input_tokens":CONTEXT-u64::from(OUTPUT),
        "max_input_bytes":INPUT_BYTES,"modes":["thinking"]});
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

fn qualified(body: &Value, own_generation: bool) -> bool {
    let info = &body["psionic"];
    body["data"]
        .as_array()
        .is_some_and(|models| models.len() == 1 && models[0]["id"] == "RecursiveSI/flint")
        && info["revision"] == REVISION
        && info["artifact"] == ARTIFACT
        && info["numerical_profile"] == PROFILE
        && info["backend"] == "cuda"
        && info["max_context_tokens"]
            .as_u64()
            .is_some_and(|n| n >= CONTEXT)
        && (info["busy"] == false || (own_generation && info["busy"] == true))
}

pub struct Native {
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
            inner: Psionic::new(base, &advertised())?
                .with_sampling(Sampling::new(1.0, 0.95, 42)?)
                .with_top_k(20)
                .with_request_timeout(DEADLINE + Duration::from_secs(10)),
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
            if max_tokens != OUTPUT {
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
        assert!(qualified(&body, false));
        let mut busy = body.clone();
        busy["psionic"]["busy"] = json!(true);
        assert!(!qualified(&busy, false));
        assert!(qualified(&busy, true));
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
            assert!(!qualified(&changed, false), "{key}");
        }
        assert!(Native::connect("http://0.0.0.0:8080").is_err());
    }
}
