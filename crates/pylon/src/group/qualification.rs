//! Provider claims tied to measured numerical evidence and the running group.
use super::*;

pub const NUMERICAL_QUALIFICATION: &str = "numerical_qualification";

/// Hashes identify the retained G1, G2, and G4 reports. This is an attributable
/// provider claim; a beacon is not remote attestation of a computation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumericalQualification {
    pub schema: String,
    pub artifact: String,
    pub manifest: String,
    pub profile: String,
    pub bundle: String,
    pub execution_config_sha256: String,
    pub max_context_tokens: u64,
    pub max_input_tokens: u64,
    pub max_output_tokens: u32,
    pub evidence: BTreeMap<String, String>,
}

impl NumericalQualification {
    /// Checks the receipt's required identities and evidence references.
    pub fn validate(&self) -> Result<(), String> {
        let sha = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
        if self.schema != "openagents.deepseek_v41.numerical_qualification.v1"
            || !self.artifact.strip_prefix("sha256:").is_some_and(sha)
            || !self.manifest.strip_prefix("sha256:").is_some_and(sha)
            || !sha(&self.bundle)
            || !sha(&self.execution_config_sha256)
            || self.profile.is_empty()
            || self.max_context_tokens < 2
            || self.max_input_tokens == 0
            || self.max_input_tokens > self.max_context_tokens
            || self.max_output_tokens == 0
            || !["G1", "G2", "G4"]
                .into_iter()
                .all(|g| self.evidence.get(g).is_some_and(|s| sha(s)))
        {
            return Err("Numerical qualification needs its model, configuration, limits, and G1, G2, and G4 evidence.".into());
        }
        Ok(())
    }
    fn matches(&self, spec: &GroupSpec, ready: &dyn GroupReadiness) -> bool {
        let stages = ready.stages();
        let devices = ready.stage_devices();
        self.validate().is_ok()
            && self.artifact == spec.artifact
            && spec.max_output_tokens <= self.max_output_tokens
            && ready.status().readiness == Readiness::Ready
            && !stages.is_empty()
            && stages.iter().all(|(_, r)| *r)
            && devices.len() == stages.len()
            && stages
                .iter()
                .all(|(s, _)| devices.iter().filter(|(n, _)| n == s).count() == 1)
            && devices.iter().all(|(_, d)| {
                d.bundle.as_ref() == Some(&self.bundle)
                    && d.profile.as_ref() == Some(&self.profile)
                    && d.manifest.as_ref() == Some(&self.manifest)
                    && d.execution_config_sha256.as_ref() == Some(&self.execution_config_sha256)
            })
            && ready.limits().is_some_and(|l| {
                l.max_context_tokens == self.max_context_tokens
                    && l.max_input_tokens == self.max_input_tokens
            })
    }
}

pub(super) fn set(beacon: &mut Beacon, spec: &GroupSpec, ready: &dyn GroupReadiness) {
    let mut meta = match beacon.meta.take() {
        Some(serde_json::Value::Object(m)) => m,
        None => serde_json::Map::new(),
        Some(other) => {
            beacon.meta = Some(other);
            return;
        }
    };
    let receipt = spec
        .numerical_qualification
        .as_ref()
        .filter(|q| q.matches(spec, ready));
    meta.insert(
        NUMERICAL_QUALIFICATION.into(),
        match receipt {
            Some(q) => serde_json::json!({"state":"qualified","receipt":q}),
            None => serde_json::json!({"state":"unqualified"}),
        },
    );
    beacon.meta = Some(serde_json::Value::Object(meta));
}

/// Whether this beacon makes a complete numerical qualification claim for its
/// own artifact. Unknown or legacy `qualified` execution labels do not qualify.
#[must_use]
pub fn numerically_qualified(beacon: &Beacon) -> bool {
    let Some(g) = beacon.group_head() else {
        return false;
    };
    if g.readiness != Some(Readiness::Ready) {
        return false;
    }
    let Some(value) = beacon
        .meta
        .as_ref()
        .and_then(|m| m.get(NUMERICAL_QUALIFICATION))
    else {
        return false;
    };
    if value["state"] != "qualified" {
        return false;
    }
    let Ok(q) = serde_json::from_value::<NumericalQualification>(value["receipt"].clone()) else {
        return false;
    };
    q.validate().is_ok()
        && q.artifact == g.artifact
        && g.max_output_tokens
            .is_some_and(|n| n <= q.max_output_tokens)
        && request_limits(beacon).is_some_and(|l| {
            l.max_context_tokens == q.max_context_tokens && l.max_input_tokens == q.max_input_tokens
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::tests::{base, ready, spec};
    use crate::identity::Identity;
    struct Running {
        device: StageDevice,
        limits: crate::stage_group::StageLimits,
    }
    impl GroupReadiness for Running {
        fn status(&self) -> GroupStatus {
            ready(7)
        }
        fn stages(&self) -> Vec<(String, bool)> {
            vec![("s0".into(), true)]
        }
        fn stage_devices(&self) -> Vec<(String, StageDevice)> {
            vec![("s0".into(), self.device.clone())]
        }
        fn limits(&self) -> Option<crate::stage_group::StageLimits> {
            Some(self.limits.clone())
        }
    }
    #[test]
    fn numerical_evidence_tracks_actual_workers_and_explicit_routing() {
        let artifact = format!("sha256:{}", "a".repeat(64));
        let mut spec = spec("v41-test", &artifact);
        let q = NumericalQualification {
            schema: "openagents.deepseek_v41.numerical_qualification.v1".into(),
            artifact: artifact.clone(),
            manifest: format!("sha256:{}", "b".repeat(64)),
            profile: "cuda-fp8-mma-tp8-sm_120".into(),
            bundle: "c".repeat(64),
            execution_config_sha256: "d".repeat(64),
            max_context_tokens: 8192,
            max_input_tokens: 2872,
            max_output_tokens: 4096,
            evidence: [
                ("G1".into(), "1".repeat(64)),
                ("G2".into(), "2".repeat(64)),
                ("G4".into(), "4".repeat(64)),
            ]
            .into(),
        };
        let limits = crate::stage_group::StageLimits {
            max_context_tokens: 8192,
            max_input_tokens: 2872,
            max_input_bytes: 100000,
            max_prefill_tokens: 2872,
            max_step_tokens: 2872,
            prefill_chunk_tokens: None,
            modes: vec![
                crate::stage_group::PromptMode::Chat,
                crate::stage_group::PromptMode::Thinking,
            ],
        };
        let mut running = Running {
            device: StageDevice {
                profile: Some(q.profile.clone()),
                bundle: Some(q.bundle.clone()),
                manifest: Some(q.manifest.clone()),
                execution_config_sha256: Some(q.execution_config_sha256.clone()),
                ..Default::default()
            },
            limits,
        };
        let key = Identity::generate();
        let base = base(&key, "v41-head", "x");
        let want = Want::Profiled(Box::new(Want::Artifact(artifact)), "qualified".into());
        let make = |s: &GroupSpec, r: &Running| {
            let mut b = head_beacon(&base, s, r, false);
            set_request_limits(&mut b, r.limits().as_ref());
            b
        };
        let legacy = make(&spec, &running);
        assert!(!numerically_qualified(&legacy));
        assert!(
            serves_want(
                &legacy,
                super::super::tests::AT,
                &BTreeSet::new(),
                Lane::CjConversation,
                &want
            )
            .is_none()
        );
        spec.numerical_qualification = Some(q);
        let good = make(&spec, &running);
        assert!(numerically_qualified(&good));
        assert!(
            serves_want(
                &good,
                super::super::tests::AT,
                &BTreeSet::new(),
                Lane::CjConversation,
                &want
            )
            .is_some()
        );
        let stage = stage_beacon(&good, &spec, 7, Layers { from: 0, to: 40 });
        assert!(!numerically_qualified(&stage));
        for field in ["bundle", "profile", "manifest", "execution"] {
            let old = running.device.clone();
            match field {
                "bundle" => running.device.bundle = Some("e".repeat(64)),
                "profile" => running.device.profile = Some("consumer-q".into()),
                "manifest" => running.device.manifest = None,
                _ => running.device.execution_config_sha256 = None,
            }
            assert!(!numerically_qualified(&make(&spec, &running)), "{field}");
            running.device = old;
        }
        running.limits.max_input_tokens += 1;
        assert!(!numerically_qualified(&make(&spec, &running)));
        let mut incomplete = spec.numerical_qualification.unwrap();
        incomplete.evidence.remove("G4");
        assert!(incomplete.validate().is_err());
    }
}
