//! Bind release qualification to a prospective corpus and actual execution.
use super::*;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Scope {
    schema: String,
    weights: String,
    manifest: String,
    numerical_profile: String,
    stage_bundle_sha256: String,
    execution_config_sha256: String,
    corpus_sha256: String,
    cases: Vec<String>,
}

fn read(opts: &QualifyOptions) -> Result<Scope, Refused> {
    let path = opts
        .policy
        .as_ref()
        .ok_or_else(|| refused("release needs a frozen policy"))?;
    let value: Value =
        serde_json::from_slice(&std::fs::read(path).map_err(|e| refused(e.to_string()))?)
            .map_err(|e| refused(e.to_string()))?;
    let scope: Scope = serde_json::from_value(value["release_scope"].clone())
        .map_err(|e| refused(format!("missing or malformed release scope: {e}")))?;
    let sha = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
    if scope.schema != "psionic.deepseek_v41.release_scope.v1"
        || scope.cases.is_empty()
        || scope.cases.iter().collect::<BTreeSet<_>>().len() != scope.cases.len()
        || ![
            &scope.stage_bundle_sha256,
            &scope.execution_config_sha256,
            &scope.corpus_sha256,
        ]
        .into_iter()
        .all(|s| sha(s))
    {
        return Err(refused("invalid release scope schema, hashes, or cases"));
    }
    Ok(scope)
}

pub(super) fn preflight(opts: &QualifyOptions) -> Result<(), Refused> {
    let scope = read(opts)?;
    let path = opts
        .corpus
        .as_ref()
        .ok_or_else(|| refused("release needs --corpus"))?;
    if checked_sha256(path)? != scope.corpus_sha256
        || opts.manifest != scope.manifest
        || opts.worker_build.as_ref() != Some(&scope.stage_bundle_sha256)
        || opts.cases.as_ref().is_none_or(|cs| {
            cs.len() != scope.cases.len()
                || cs.iter().collect::<BTreeSet<_>>() != scope.cases.iter().collect::<BTreeSet<_>>()
        })
    {
        return Err(refused(
            "corpus, cases, manifest, or bundle differ from the frozen release scope",
        ));
    }
    let corpus: Value =
        serde_json::from_slice(&std::fs::read(path).map_err(|e| refused(e.to_string()))?)
            .map_err(|e| refused(e.to_string()))?;
    let names: Vec<_> = corpus["cases"]
        .as_array()
        .ok_or_else(|| refused("corpus has no cases"))?
        .iter()
        .map(|c| {
            c["id"]
                .as_str()
                .ok_or_else(|| refused("corpus case has no ID"))
        })
        .collect::<Result<_, _>>()?;
    if names.len() != scope.cases.len()
        || names.iter().copied().collect::<BTreeSet<_>>()
            != scope
                .cases
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
    {
        return Err(refused("the corpus does not name exactly the frozen cases"));
    }
    let oracle = OracleDump::open(&opts.oracle)?;
    for name in &scope.cases {
        if !oracle.run_hashes.contains_key(&format!("{name}/run.json"))
            || oracle.run(name)?["corpus_sha256"] != scope.corpus_sha256
        {
            return Err(refused(format!(
                "{name}: oracle capture is not bound to this corpus"
            )));
        }
    }
    Ok(())
}

pub(super) fn violations(opts: &QualifyOptions, group: &Group) -> Vec<String> {
    let scope = match read(opts) {
        Ok(s) => s,
        Err(e) => return vec![e.to_string()],
    };
    let mut out = Vec::new();
    if group.weights != scope.weights
        || group.profile != scope.numerical_profile
        || group.build != scope.stage_bundle_sha256
    {
        out.push(
            "actual weights, numerical profile, or bundle differ from the release scope".into(),
        );
    }
    if group.execution_configs.is_empty()
        || group
            .execution_configs
            .iter()
            .any(|c| c != &scope.execution_config_sha256)
    {
        out.push("actual stage configuration is missing or differs from the release scope".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corpus_cases_bundle_and_run_metadata_are_bound_before_loading() {
        let root = tempfile::tempdir().unwrap();
        let corpus = root.path().join("corpus.json");
        let policy = root.path().join("policy.json");
        std::fs::write(&corpus, r#"{"cases":[{"id":"a"},{"id":"b"}]}"#).unwrap();
        let corpus_sha = checked_sha256(&corpus).unwrap();
        let mut hashes = BTreeMap::new();
        for name in ["a", "b"] {
            std::fs::create_dir(root.path().join(name)).unwrap();
            let path = root.path().join(name).join("run.json");
            std::fs::write(
                &path,
                serde_json::to_vec(&json!({"corpus_sha256":corpus_sha})).unwrap(),
            )
            .unwrap();
            hashes.insert(format!("{name}/run.json"), checked_sha256(&path).unwrap());
        }
        std::fs::write(
            root.path().join("manifest.json"),
            serde_json::to_vec(&json!({"tensors":[],"run_files_sha256":hashes})).unwrap(),
        )
        .unwrap();
        std::fs::write(&policy,serde_json::to_vec(&json!({"release_scope":{
            "schema":"psionic.deepseek_v41.release_scope.v1","weights":"w","manifest":"m","numerical_profile":"cuda-device-sm_120",
            "stage_bundle_sha256":"a".repeat(64),"execution_config_sha256":"b".repeat(64),"corpus_sha256":corpus_sha,"cases":["a","b"]}})).unwrap()).unwrap();
        let mut o = QualifyOptions::parse(&[
            "--weights".into(),
            "w".into(),
            "--manifest".into(),
            "m".into(),
            "--oracle".into(),
            root.path().display().to_string(),
            "--out".into(),
            root.path().join("out").display().to_string(),
            "--worker-build".into(),
            "a".repeat(64),
            "--cases".into(),
            "a,b".into(),
            "--policy".into(),
            policy.display().to_string(),
            "--corpus".into(),
            corpus.display().to_string(),
            "--release".into(),
        ])
        .unwrap();
        preflight(&o).unwrap();
        o.cases = Some(vec!["a".into()]);
        assert!(preflight(&o).is_err());
        o.cases = Some(vec!["a".into(), "b".into()]);
        o.worker_build = Some("c".repeat(64));
        assert!(preflight(&o).is_err());
        o.worker_build = Some("a".repeat(64));
        std::fs::write(root.path().join("a/run.json"), b"{}").unwrap();
        assert!(preflight(&o).is_err());
        std::fs::write(&corpus, b"{}").unwrap();
        assert!(preflight(&o).is_err());
    }
    #[test]
    fn configuration_hash_tracks_numerics_and_effective_limits() {
        let wire = json!({"cache":"bf16","residual":"f32"});
        let settings =
            json!({"weights":"original","experts":"device","prefill":"token","cache":"unpacked"});
        let limits = json!({"max_context_tokens":8192,"max_input_tokens":2872});
        let f = |p: &str, m: &str, w: &Value, l: &Value, s: &Value| {
            crate::execution_fingerprint(p, m, w, l, s)
        };
        let a = f("cuda-device-sm_120", "m", &wire, &limits, &settings).unwrap();
        for b in [
            f("cuda-fp8-mma-tp8-sm_120", "m", &wire, &limits, &settings),
            f("cuda-device-sm_120", "n", &wire, &limits, &settings),
            f(
                "cuda-device-sm_120",
                "m",
                &json!({"cache":"f32"}),
                &limits,
                &settings,
            ),
            f(
                "cuda-device-sm_120",
                "m",
                &wire,
                &json!({"max_context_tokens":8192,"max_input_tokens":8192}),
                &settings,
            ),
        ] {
            assert_ne!(Some(&a), b.as_ref());
        }
        assert!(f("p", "m", &wire, &limits, &Value::Null).is_none());
    }
}
