//! Mount existing tracker and LAB owners into the native WORK authority.
use serde_json::Value;
use std::path::Path;
pub(crate) fn run(args: Vec<String>) -> Result<Value, String> {
    let option = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let github = if let Some(path) = option("--tracker") {
        let snapshot = coder::tracker::Snapshot::load(Path::new(&path))?;
        let node = option("--node").ok_or("GitHub node identity is required")?;
        let issue = snapshot
            .issues
            .iter()
            .find(|i| i.node == node)
            .ok_or("GitHub node is absent from the validated tracker snapshot")?;
        let captured = snapshot
            .fetched
            .parse::<u64>()
            .map_err(|_| "tracker capture must record its Unix observation time")?;
        let mut capture = serde_json::to_value(issue).map_err(|e| e.to_string())?;
        capture["tracker_snapshot"] = serde_json::json!({"digest":snapshot.digest(),"scope":snapshot.scope,"base":snapshot.base,"body_coverage":"excerpt_only"});
        Some((capture, captured))
    } else {
        None
    };
    let labor_state = option("--labor-state");
    let labor_setup = option("--labor-setup");
    work_host::cli::run_configured(
        args,
        move |_, secret, _| match (labor_state, labor_setup) {
            (None, None) => Ok(None),
            (Some(dir), Some(file)) => {
                if !Path::new(&dir).join("labor.json").is_file() {
                    return Err("an existing admitted LAB store is required".into());
                }
                let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
                let setup: coder_labor::book::Setup = serde_json::from_value(
                    nostr::contracts::parse_strict_bounded(&bytes, 8 * 1_048_576)
                        .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                let store = coder_labor::store::Store::open(Path::new(&dir), setup, *secret)?;
                Ok(Some(Box::new(coder_labor::work::Adapter::new(store)?)))
            }
            _ => Err("LAB linking requires both its state directory and admitted setup".into()),
        },
        github,
    )
}
