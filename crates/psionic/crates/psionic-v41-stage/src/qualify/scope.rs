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
    parse(&value)
}

fn parse(document: &Value) -> Result<Scope, Refused> {
    let scope: Scope = serde_json::from_value(document["release_scope"].clone())
        .map_err(|e| refused(format!("missing or malformed release scope: {e}")))?;
    let sha = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
    if scope.schema != "psionic.deepseek_v41.release_scope.v1"
        || scope.cases.is_empty()
        || scope.cases.iter().collect::<BTreeSet<_>>().len() != scope.cases.len()
        || scope.numerical_profile.is_empty()
        || [&scope.weights, &scope.manifest]
            .iter()
            .any(|s| s.strip_prefix("sha256:").is_none_or(|s| !sha(s)))
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
    let document: Value = serde_json::from_slice(
        &std::fs::read(opts.policy.as_ref().unwrap()).map_err(|e| refused(e.to_string()))?,
    )
    .map_err(|e| refused(e.to_string()))?;
    confirmation_inputs(&document, path, &opts.oracle)?;
    Ok(())
}

/// Verify the frozen corpus and hash-bound oracle lengths independently of reports.
pub(super) fn confirmation_inputs(
    document: &Value,
    corpus_path: &Path,
    oracle_root: &Path,
) -> Result<BTreeMap<String, (u64, u64)>, Refused> {
    let scope = &document["release_scope"];
    parse(document)?;
    if checked_sha256(corpus_path)? != scope["corpus_sha256"] {
        return Err(refused("corpus differs from the frozen release scope"));
    }
    let corpus: Value =
        serde_json::from_slice(&std::fs::read(corpus_path).map_err(|e| refused(e.to_string()))?)
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
    let frozen = scope["cases"]
        .as_array()
        .ok_or_else(|| refused("missing frozen cases"))?;
    if names.len() != frozen.len()
        || names.iter().copied().collect::<BTreeSet<_>>().len() != names.len()
        || names.iter().copied().collect::<BTreeSet<_>>()
            != frozen
                .iter()
                .filter_map(Value::as_str)
                .collect::<BTreeSet<_>>()
    {
        return Err(refused("the corpus does not name exactly the frozen cases"));
    }
    let oracle = OracleDump::open(oracle_root)?;
    let mut lengths = BTreeMap::new();
    for case in corpus["cases"].as_array().unwrap() {
        let name = case["id"].as_str().unwrap();
        let run = oracle.run(name)?;
        let prompt: Vec<u32> = serde_json::from_value(run["prompt_token_ids"].clone())
            .map_err(|e| refused(format!("{name}: invalid oracle prompt IDs: {e}")))?;
        let generated: Vec<u32> = serde_json::from_value(run["generated_ids"].clone())
            .map_err(|e| refused(format!("{name}: invalid oracle generated IDs: {e}")))?;
        let n_gen = case["n_gen"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= 32768)
            .ok_or_else(|| refused(format!("{name}: invalid frozen generation length")))?;
        let labeled: BTreeSet<_> = run["labels"]
            .as_object()
            .into_iter()
            .flat_map(|labels| labels.values().filter_map(Value::as_u64))
            .collect();
        if !oracle.run_hashes.contains_key(&format!("{name}/run.json"))
            || run["corpus_sha256"] != scope["corpus_sha256"]
            || run["id"] != name
            || prompt.is_empty()
            || run["prompt_tokens"].as_u64() != Some(prompt.len() as u64)
            || run["n_gen"].as_u64() != Some(n_gen)
            || generated.len() as u64 != n_gen
            || case["all_generated_rows"] == true
                && (prompt.len() as u64..prompt.len() as u64 + n_gen)
                    .any(|position| !labeled.contains(&position))
            || case
                .get("cut")
                .is_some_and(|cut| cut.as_u64() != Some(prompt.len() as u64))
        {
            return Err(refused(format!(
                "{name}: oracle capture lengths or identity differ from the frozen corpus"
            )));
        }
        lengths.insert(name.to_owned(), (prompt.len() as u64, n_gen));
    }
    Ok(lengths)
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
        std::fs::write(
            &corpus,
            r#"{"cases":[{"id":"a","n_gen":2,"cut":1},{"id":"b","n_gen":2}]}"#,
        )
        .unwrap();
        let corpus_sha = checked_sha256(&corpus).unwrap();
        let mut hashes = BTreeMap::new();
        for name in ["a", "b"] {
            std::fs::create_dir(root.path().join(name)).unwrap();
            let path = root.path().join(name).join("run.json");
            std::fs::write(
                &path,
                serde_json::to_vec(&json!({"id":name,"corpus_sha256":corpus_sha,"n_gen":2,"prompt_tokens":1,"prompt_token_ids":[1],"generated_ids":[2,3]})).unwrap(),
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
            "schema":"psionic.deepseek_v41.release_scope.v1","weights":format!("sha256:{}", "0".repeat(64)),"manifest":format!("sha256:{}", "1".repeat(64)),"numerical_profile":"cuda-device-sm_120",
            "stage_bundle_sha256":"a".repeat(64),"execution_config_sha256":"b".repeat(64),"corpus_sha256":corpus_sha,"cases":["a","b"]}})).unwrap()).unwrap();
        let mut o = QualifyOptions::parse(&[
            "--weights".into(),
            "w".into(),
            "--manifest".into(),
            format!("sha256:{}", "1".repeat(64)),
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
        // Even a freshly hashed capture cannot shorten the frozen continuation.
        let path = root.path().join("a/run.json");
        let old = std::fs::read(&path).unwrap();
        std::fs::write(&path, serde_json::to_vec(&json!({"id":"a","corpus_sha256":corpus_sha,"n_gen":0,"prompt_tokens":1,"prompt_token_ids":[1],"generated_ids":[]})).unwrap()).unwrap();
        hashes.insert("a/run.json".into(), checked_sha256(&path).unwrap());
        std::fs::write(
            root.path().join("manifest.json"),
            serde_json::to_vec(&json!({"tensors":[],"run_files_sha256":hashes})).unwrap(),
        )
        .unwrap();
        assert!(preflight(&o).is_err());
        std::fs::write(&path, old).unwrap();
        hashes.insert("a/run.json".into(), checked_sha256(&path).unwrap());
        std::fs::write(
            root.path().join("manifest.json"),
            serde_json::to_vec(&json!({"tensors":[],"run_files_sha256":hashes})).unwrap(),
        )
        .unwrap();
        preflight(&o).unwrap();
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
