//! The Labs worker's Robot `simulate` job (`robot-workbench simulate-job`).
//!
//! A job's input names its experiment version first, then the scene and
//! robot versions it references (each named by platform content digest),
//! and the stored files those versions use, already downloaded and checked
//! by the worker. [`materialize`] writes them into a fresh local Robot Lab
//! project, rewriting each reference to the local version, so the run uses
//! exactly the checks and runner the local lab uses. [`simulate_job`] runs
//! the experiment in MuJoCo and answers with the replay: the recorded
//! episode, naming its experiment by platform digest again.
use crate::experiments::{Config, Session};
use crate::host::Host;
use base64::Engine as _;
use robot_lab::{Document, Reference, Store};
use robot_sim::runner::Mode;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use robot_lab::kind::REPLAY_MEDIA;

/// A replay larger than this can't be returned in one job result.
pub const MAX_REPLAY: usize = 8 * 1024 * 1024;

/// A job's experiment, saved in a local project.
pub struct Materialized {
    pub store: Store,
    pub experiment: Reference,
    /// The experiment's platform document and content digest.
    pub platform: Reference,
    pub seed: u64,
}

fn rank(document: &Document) -> u8 {
    match document {
        Document::Robot(_) => 0,
        Document::Scene(_) => 1,
        Document::Experiment(_) => 2,
        Document::Episode(_) => 3,
    }
}

fn paths(input: &Value, field: &str) -> Result<BTreeMap<String, PathBuf>, String> {
    serde_json::from_value(input.get(field).cloned().unwrap_or_else(|| json!({})))
        .map_err(|_| "Read the simulation job's files.".to_owned())
}

/// Saves the job's versions and files under `root` and returns its experiment.
///
/// # Errors
/// The input isn't a simulation job, a file doesn't match its checksum, a
/// reference names a version the job can't read, or a local check fails.
pub fn materialize(input: &Value, root: &Path) -> Result<Materialized, String> {
    let store = Store::open(root, "labs-job").map_err(|e| e.to_string())?;
    let mut files = paths(input, "files")?;
    files.extend(paths(input, "local_assets")?);
    for (sha256, path) in &files {
        let file = std::fs::File::open(path).map_err(|_| "A job file is missing.".to_owned())?;
        let asset = store
            .put_asset(file, "application/octet-stream", 4 << 30)
            .map_err(|e| e.to_string())?;
        if asset.sha256 != *sha256 {
            return Err("A job file doesn't match the version that uses it.".into());
        }
    }
    let mut versions = Vec::new();
    for value in input["versions"].as_array().into_iter().flatten() {
        let document: Document = serde_json::from_value(value["content"].clone())
            .map_err(|_| "Choose a robot, scene, or experiment.".to_owned())?;
        let platform = Reference {
            document: value["document"].as_str().unwrap_or_default().into(),
            version: value["content_sha256"].as_str().unwrap_or_default().into(),
        };
        versions.push((platform, document));
    }
    let (platform, root_document) = versions
        .first()
        .cloned()
        .ok_or("Choose the experiment to simulate.")?;
    let Document::Experiment(experiment) = &root_document else {
        return Err("Choose an experiment to simulate.".into());
    };
    let seed = match input["work"]["assignment"]["lease"]["job"]["spec"]["params"]["seed"].as_u64()
    {
        Some(seed) if experiment.seeds.contains(&seed) => seed,
        Some(_) => return Err("Choose a seed from this experiment.".into()),
        None => *experiment
            .seeds
            .first()
            .ok_or("This experiment has no seeds.")?,
    };
    versions.sort_by_key(|(_, d)| rank(d));
    let mut local: BTreeMap<Reference, Reference> = BTreeMap::new();
    for (index, (platform, mut document)) in versions.into_iter().enumerate() {
        if local.contains_key(&platform) {
            continue;
        }
        for reference in document.references_mut() {
            *reference = local
                .get(reference)
                .cloned()
                .ok_or("This item refers to a version the job can't read.")?;
        }
        let saved = store
            .save(
                &format!("item-{index}"),
                None,
                document,
                "Labs worker",
                "Saved for a simulation",
                0,
            )
            .map_err(|e| e.to_string())?;
        local.insert(platform, saved);
    }
    Ok(Materialized {
        experiment: local
            .get(&platform)
            .cloned()
            .ok_or("Choose the experiment to simulate.")?,
        store,
        platform,
        seed,
    })
}

/// The recorded episode as platform content, naming `platform` as its experiment.
///
/// # Errors
/// The episode can't be read.
pub fn replay(store: &Store, episode: &Reference, platform: &Reference) -> Result<Value, String> {
    let mut document = store.get(episode).map_err(|e| e.to_string())?.content;
    let Document::Episode(recorded) = &mut document else {
        return Err("The simulation didn't record an episode.".into());
    };
    recorded.experiment = platform.clone();
    serde_json::to_value(&document).map_err(|e| e.to_string())
}

/// The handler output for a replay.
///
/// # Errors
/// The replay is too large to return.
pub fn output(replay: &Value) -> Result<Value, String> {
    let bytes = serde_json::to_vec(replay).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_REPLAY {
        return Err("The replay is too large to return.".into());
    }
    Ok(json!({"outputs": [{
        "role": "replay",
        "media_type": REPLAY_MEDIA,
        "content_base64": base64::engine::general_purpose::STANDARD.encode(bytes),
    }]}))
}

/// Runs the job on stdin's input under `dir` with the operator's MuJoCo
/// and robot catalog, and returns the handler output.
///
/// # Errors
/// The job can't be read or saved locally, or the simulation fails.
pub fn simulate_job(
    input: &[u8],
    config: &Config,
    catalog: &Path,
    dir: &Path,
) -> Result<Value, String> {
    let input: Value =
        serde_json::from_slice(input).map_err(|_| "Read the simulation job.".to_owned())?;
    let job = input["work"]["assignment"]["lease"]["job"]["id"]
        .as_str()
        .filter(|id| id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        .unwrap_or("labs-job")
        .to_owned();
    let saved = materialize(&input, &dir.join("robot"))?;
    let host = Host::open(saved.store.clone(), catalog, "http://127.0.0.1:8794")?;
    let mut session = Session::start(
        host,
        saved.experiment,
        saved.seed,
        &job,
        Mode::Batch,
        config,
    )?;
    let episode = session.run()?;
    output(&replay(&saved.store, &episode, &saved.platform)?)
}

/// `robot-workbench simulate-job --library PATH --library-sha SHA [--catalog DIR]`
///
/// # Errors
/// The options are wrong, or the job fails.
pub fn cli(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{Read, Write};
    let mut values = BTreeMap::new();
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        let value = args.next().ok_or("Choose a value for this option")?;
        if !["--library", "--library-sha", "--catalog"].contains(&flag.as_str())
            || values.insert(flag, value).is_some()
        {
            return Err("Use --library, --library-sha, and --catalog once each".into());
        }
    }
    let config = Config {
        library: robot_sim::mujoco::LibraryPin {
            path: values.get("--library").ok_or("Choose --library")?.into(),
            sha256: values
                .get("--library-sha")
                .ok_or("Choose --library-sha")?
                .clone(),
        },
        policy: None,
    };
    let catalog = values.get("--catalog").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101"),
        PathBuf::from,
    );
    let mut input = Vec::new();
    std::io::stdin()
        .take(16 * 1024 * 1024)
        .read_to_end(&mut input)?;
    let dir = std::env::current_dir()?;
    let output = simulate_job(&input, &config, &catalog, &dir)?;
    std::io::stdout().write_all(&serde_json::to_vec(&output)?)?;
    Ok(())
}

#[cfg(test)]
mod tests;
