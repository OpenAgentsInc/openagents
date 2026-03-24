use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use psionic_train::{
    RemoteTrainingResultClassification, RemoteTrainingRunIndex, RemoteTrainingVisualizationBundle,
};

use crate::app_state::RenderState;

const REMOTE_TRAINING_INDEX_RELATIVE_PATH: &str =
    "fixtures/training_visualization/remote_training_run_index_v1.json";
const REMOTE_TRAINING_BUNDLE_CACHE_DIR: &str = "bundles";
const REMOTE_TRAINING_IDLE_REFRESH_INTERVAL_MS: u64 = 15_000;
const REMOTE_TRAINING_ACTIVE_REFRESH_INTERVAL_MS: u64 = 1_000;

pub const OPENAGENTS_REMOTE_TRAINING_SOURCE_ROOT_ENV: &str =
    "OPENAGENTS_REMOTE_TRAINING_SOURCE_ROOT";
pub const OPENAGENTS_REMOTE_TRAINING_INDEX_PATH_ENV: &str = "OPENAGENTS_REMOTE_TRAINING_INDEX_PATH";
pub const OPENAGENTS_REMOTE_TRAINING_CACHE_ROOT_ENV: &str = "OPENAGENTS_REMOTE_TRAINING_CACHE_ROOT";

struct LoadedRemoteTrainingMirror {
    source_root: Option<PathBuf>,
    source_index_path: Option<PathBuf>,
    using_cached_mirror: bool,
    fallback_warning: Option<String>,
    run_index: RemoteTrainingRunIndex,
    bundles: BTreeMap<String, RemoteTrainingVisualizationBundle>,
    mirrored_bundle_paths: BTreeMap<String, PathBuf>,
}

pub(crate) fn default_remote_training_source_root_hint() -> Option<PathBuf> {
    std::env::var(OPENAGENTS_REMOTE_TRAINING_SOURCE_ROOT_ENV)
        .ok()
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| {
            let candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../psionic");
            candidate.exists().then_some(candidate)
        })
}

pub(crate) fn default_remote_training_source_index_path_hint() -> Option<PathBuf> {
    std::env::var(OPENAGENTS_REMOTE_TRAINING_INDEX_PATH_ENV)
        .ok()
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| {
            default_remote_training_source_root_hint()
                .map(|root| root.join(REMOTE_TRAINING_INDEX_RELATIVE_PATH))
                .filter(|path| path.exists())
        })
}

pub(crate) fn default_remote_training_cache_root() -> PathBuf {
    if let Ok(path) = std::env::var(OPENAGENTS_REMOTE_TRAINING_CACHE_ROOT_ENV) {
        return PathBuf::from(path);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openagents")
        .join("remote-training-v1")
}

pub(crate) fn refresh_remote_training_sync_cache_if_due(
    state: &mut RenderState,
    force: bool,
) -> bool {
    let now_epoch_ms = current_epoch_ms();
    let sync = &state.desktop_control.remote_training;
    let due = force
        || sync.last_refreshed_at_epoch_ms.is_none_or(|last| {
            now_epoch_ms.saturating_sub(last) >= sync.refresh_interval_ms.max(1)
        });
    if !due {
        return false;
    }

    let previous_index = sync.run_index.clone();
    let previous_bundles = sync.bundles.clone();
    let previous_source_root = sync.source_root.clone();
    let previous_source_index_path = sync.source_index_path.clone();
    let previous_cache_paths = sync.mirrored_bundle_paths.clone();
    let previous_selection = sync.selected_run_id.clone();
    let previous_last_error = sync.last_error.clone();
    let previous_last_action = sync.last_action.clone();
    let previous_cached = sync.using_cached_mirror;
    let previous_last_successful = sync.last_successful_sync_at_epoch_ms;
    let previous_interval = sync.refresh_interval_ms;

    let result = load_remote_training_live_or_cached(
        state
            .desktop_control
            .remote_training
            .source_root_hint
            .as_deref(),
        state
            .desktop_control
            .remote_training
            .source_index_path_hint
            .as_deref(),
        state.desktop_control.remote_training.cache_root.as_path(),
    );

    let sync = &mut state.desktop_control.remote_training;
    let mut changed = false;
    match result {
        Ok(loaded) => {
            let selection = next_selected_run_id(
                loaded.run_index.entries.as_slice(),
                &loaded.bundles,
                sync.selected_run_id.as_deref(),
            );
            let refresh_interval_ms =
                refresh_interval_for_runs(loaded.run_index.entries.as_slice(), &loaded.bundles);
            changed |= previous_index.as_ref() != Some(&loaded.run_index);
            changed |= previous_bundles != loaded.bundles;
            changed |= previous_source_root != loaded.source_root;
            changed |= previous_source_index_path != loaded.source_index_path;
            changed |= previous_cache_paths != loaded.mirrored_bundle_paths;
            changed |= previous_selection != selection;
            changed |= previous_cached != loaded.using_cached_mirror;
            changed |= previous_last_error != loaded.fallback_warning;
            changed |= previous_interval != refresh_interval_ms;
            sync.source_root = loaded.source_root;
            sync.source_index_path = loaded.source_index_path;
            sync.using_cached_mirror = loaded.using_cached_mirror;
            sync.run_index = Some(loaded.run_index);
            sync.bundles = loaded.bundles;
            sync.mirrored_bundle_paths = loaded.mirrored_bundle_paths;
            sync.selected_run_id = selection;
            sync.refresh_interval_ms = refresh_interval_ms;
            sync.last_successful_sync_at_epoch_ms = Some(now_epoch_ms);
            sync.last_error = loaded.fallback_warning;
            sync.last_action = Some(format!(
                "Remote training mirror synced {} runs and {} cached bundles{}",
                sync.run_index
                    .as_ref()
                    .map(|index| index.entries.len())
                    .unwrap_or(0),
                sync.bundles.len(),
                if sync.using_cached_mirror {
                    " using the app cache"
                } else {
                    " from the live source"
                }
            ));
        }
        Err(error) => {
            let refresh_interval_ms = refresh_interval_for_runs(
                sync.run_index
                    .as_ref()
                    .map(|index| index.entries.as_slice())
                    .unwrap_or(&[]),
                &sync.bundles,
            );
            changed |= previous_last_error.as_deref() != Some(error.as_str());
            changed |=
                previous_last_action.as_deref() != Some("Remote training mirror refresh failed");
            changed |= previous_interval != refresh_interval_ms;
            sync.refresh_interval_ms = refresh_interval_ms;
            sync.last_error = Some(error);
            sync.last_action = Some("Remote training mirror refresh failed".to_string());
        }
    }
    changed |= previous_last_successful != sync.last_successful_sync_at_epoch_ms;
    changed |= sync.last_refreshed_at_epoch_ms != Some(now_epoch_ms);
    sync.last_refreshed_at_epoch_ms = Some(now_epoch_ms);
    changed
}

fn load_remote_training_live_or_cached(
    source_root_hint: Option<&Path>,
    source_index_path_hint: Option<&Path>,
    cache_root: &Path,
) -> Result<LoadedRemoteTrainingMirror, String> {
    match load_remote_training_from_source(source_root_hint, source_index_path_hint, cache_root) {
        Ok(loaded) => Ok(loaded),
        Err(source_error) => match load_remote_training_from_cache(cache_root) {
            Ok(mut cached) => {
                cached.using_cached_mirror = true;
                cached.fallback_warning = Some(source_error);
                Ok(cached)
            }
            Err(_) => Err(source_error),
        },
    }
}

fn load_remote_training_from_source(
    source_root_hint: Option<&Path>,
    source_index_path_hint: Option<&Path>,
    cache_root: &Path,
) -> Result<LoadedRemoteTrainingMirror, String> {
    let source_root = source_root_hint
        .map(PathBuf::from)
        .or_else(default_remote_training_source_root_hint);
    let source_index_path = source_index_path_hint
        .map(PathBuf::from)
        .or_else(|| {
            source_root
                .as_ref()
                .map(|root| root.join(REMOTE_TRAINING_INDEX_RELATIVE_PATH))
        })
        .or_else(default_remote_training_source_index_path_hint)
        .ok_or_else(|| {
            "remote training source is not configured; set OPENAGENTS_REMOTE_TRAINING_SOURCE_ROOT or OPENAGENTS_REMOTE_TRAINING_INDEX_PATH".to_string()
        })?;
    let raw = fs::read_to_string(&source_index_path).map_err(|error| {
        format!(
            "failed to read remote training run index {}: {error}",
            source_index_path.display()
        )
    })?;
    let run_index =
        serde_json::from_str::<RemoteTrainingRunIndex>(raw.as_str()).map_err(|error| {
            format!(
                "failed to decode remote training run index {}: {error}",
                source_index_path.display()
            )
        })?;
    run_index.validate().map_err(|error| {
        format!(
            "remote training run index {} failed validation: {error}",
            source_index_path.display()
        )
    })?;
    let bundles = load_bundles_for_index(source_root.as_deref(), &run_index)?;
    let mirrored_bundle_paths =
        persist_remote_training_cache(cache_root, &source_index_path, &run_index, &bundles)?;
    Ok(LoadedRemoteTrainingMirror {
        source_root,
        source_index_path: Some(source_index_path),
        using_cached_mirror: false,
        fallback_warning: None,
        run_index,
        bundles,
        mirrored_bundle_paths,
    })
}

fn load_remote_training_from_cache(
    cache_root: &Path,
) -> Result<LoadedRemoteTrainingMirror, String> {
    let cache_index_path = cache_root.join(REMOTE_TRAINING_INDEX_RELATIVE_PATH);
    let raw = fs::read_to_string(&cache_index_path).map_err(|error| {
        format!(
            "failed to read cached remote training run index {}: {error}",
            cache_index_path.display()
        )
    })?;
    let run_index =
        serde_json::from_str::<RemoteTrainingRunIndex>(raw.as_str()).map_err(|error| {
            format!(
                "failed to decode cached remote training run index {}: {error}",
                cache_index_path.display()
            )
        })?;
    run_index.validate().map_err(|error| {
        format!(
            "cached remote training run index {} failed validation: {error}",
            cache_index_path.display()
        )
    })?;
    let mut bundles = BTreeMap::new();
    let mut mirrored_bundle_paths = BTreeMap::new();
    for entry in &run_index.entries {
        let bundle_path = cache_bundle_path(cache_root, entry.run_id.as_str());
        if !bundle_path.exists() {
            continue;
        }
        let bundle = read_bundle(bundle_path.as_path())?;
        verify_bundle_matches_entry(
            entry.run_id.as_str(),
            entry.bundle_digest.as_deref(),
            &bundle,
        )?;
        mirrored_bundle_paths.insert(entry.run_id.clone(), bundle_path);
        bundles.insert(entry.run_id.clone(), bundle);
    }
    Ok(LoadedRemoteTrainingMirror {
        source_root: None,
        source_index_path: Some(cache_index_path),
        using_cached_mirror: true,
        fallback_warning: None,
        run_index,
        bundles,
        mirrored_bundle_paths,
    })
}

fn load_bundles_for_index(
    source_root: Option<&Path>,
    run_index: &RemoteTrainingRunIndex,
) -> Result<BTreeMap<String, RemoteTrainingVisualizationBundle>, String> {
    let mut bundles = BTreeMap::new();
    for entry in &run_index.entries {
        let Some(bundle_uri) = entry.bundle_artifact_uri.as_deref() else {
            continue;
        };
        let Some(bundle_path) = resolve_bundle_path(source_root, bundle_uri) else {
            continue;
        };
        if !bundle_path.exists() {
            continue;
        }
        let bundle = read_bundle(bundle_path.as_path())?;
        verify_bundle_matches_entry(
            entry.run_id.as_str(),
            entry.bundle_digest.as_deref(),
            &bundle,
        )?;
        bundles.insert(entry.run_id.clone(), bundle);
    }
    Ok(bundles)
}

fn persist_remote_training_cache(
    cache_root: &Path,
    source_index_path: &Path,
    run_index: &RemoteTrainingRunIndex,
    bundles: &BTreeMap<String, RemoteTrainingVisualizationBundle>,
) -> Result<BTreeMap<String, PathBuf>, String> {
    let cache_index_path = cache_root.join(REMOTE_TRAINING_INDEX_RELATIVE_PATH);
    let cache_bundle_dir = cache_root.join(REMOTE_TRAINING_BUNDLE_CACHE_DIR);
    if let Some(parent) = cache_index_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create remote training cache directory {}: {error}",
                parent.display()
            )
        })?;
    }
    fs::create_dir_all(&cache_bundle_dir).map_err(|error| {
        format!(
            "failed to create remote training bundle cache directory {}: {error}",
            cache_bundle_dir.display()
        )
    })?;
    let index_bytes = fs::read(source_index_path)
        .or_else(|_| serde_json::to_vec_pretty(run_index).map_err(std::io::Error::other))
        .map_err(|error| {
            format!(
                "failed to serialize remote training run index for cache {}: {error}",
                cache_index_path.display()
            )
        })?;
    fs::write(&cache_index_path, index_bytes).map_err(|error| {
        format!(
            "failed to write remote training cache index {}: {error}",
            cache_index_path.display()
        )
    })?;

    let mut mirrored_bundle_paths = BTreeMap::new();
    for (run_id, bundle) in bundles {
        let bundle_path = cache_bundle_path(cache_root, run_id.as_str());
        let bytes = serde_json::to_vec_pretty(bundle).map_err(|error| {
            format!(
                "failed to encode remote training bundle {} for cache: {error}",
                run_id
            )
        })?;
        fs::write(&bundle_path, bytes).map_err(|error| {
            format!(
                "failed to write remote training cached bundle {}: {error}",
                bundle_path.display()
            )
        })?;
        mirrored_bundle_paths.insert(run_id.clone(), bundle_path);
    }
    Ok(mirrored_bundle_paths)
}

fn next_selected_run_id(
    entries: &[psionic_train::RemoteTrainingRunIndexEntry],
    _bundles: &BTreeMap<String, RemoteTrainingVisualizationBundle>,
    current_selection: Option<&str>,
) -> Option<String> {
    if current_selection.is_some_and(|run_id| entries.iter().any(|entry| entry.run_id == run_id)) {
        return current_selection.map(str::to_string);
    }
    entries
        .iter()
        .find(|entry| entry.result_classification == RemoteTrainingResultClassification::Active)
        .or_else(|| entries.first())
        .map(|entry| entry.run_id.clone())
}

fn refresh_interval_for_runs(
    entries: &[psionic_train::RemoteTrainingRunIndexEntry],
    bundles: &BTreeMap<String, RemoteTrainingVisualizationBundle>,
) -> u64 {
    let mut interval_ms = REMOTE_TRAINING_IDLE_REFRESH_INTERVAL_MS;
    for entry in entries {
        if entry.result_classification != RemoteTrainingResultClassification::Active {
            continue;
        }
        let entry_interval = bundles
            .get(entry.run_id.as_str())
            .map(|bundle| bundle.refresh_contract.target_ui_update_interval_ms)
            .unwrap_or(REMOTE_TRAINING_ACTIVE_REFRESH_INTERVAL_MS);
        interval_ms = interval_ms.min(entry_interval.max(1));
    }
    interval_ms.max(1)
}

fn resolve_bundle_path(source_root: Option<&Path>, bundle_uri: &str) -> Option<PathBuf> {
    if bundle_uri.starts_with("gs://")
        || bundle_uri.starts_with("s3://")
        || bundle_uri.starts_with("http://")
        || bundle_uri.starts_with("https://")
    {
        return None;
    }
    let path = PathBuf::from(bundle_uri);
    if path.is_absolute() {
        return Some(path);
    }
    source_root.map(|root| root.join(path))
}

fn read_bundle(path: &Path) -> Result<RemoteTrainingVisualizationBundle, String> {
    let raw = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read remote training bundle {}: {error}",
            path.display()
        )
    })?;
    let bundle = serde_json::from_str::<RemoteTrainingVisualizationBundle>(raw.as_str()).map_err(
        |error| {
            format!(
                "failed to decode remote training bundle {}: {error}",
                path.display()
            )
        },
    )?;
    bundle.validate().map_err(|error| {
        format!(
            "remote training bundle {} failed validation: {error}",
            path.display()
        )
    })?;
    Ok(bundle)
}

fn verify_bundle_matches_entry(
    expected_run_id: &str,
    expected_digest: Option<&str>,
    bundle: &RemoteTrainingVisualizationBundle,
) -> Result<(), String> {
    if bundle.run_id != expected_run_id {
        return Err(format!(
            "remote training bundle run_id {} did not match expected run {}",
            bundle.run_id, expected_run_id
        ));
    }
    if let Some(expected_digest) = expected_digest
        && bundle.bundle_digest != expected_digest
    {
        return Err(format!(
            "remote training bundle digest {} did not match expected digest {} for run {}",
            bundle.bundle_digest, expected_digest, expected_run_id
        ));
    }
    Ok(())
}

fn cache_bundle_path(cache_root: &Path, run_id: &str) -> PathBuf {
    cache_root
        .join(REMOTE_TRAINING_BUNDLE_CACHE_DIR)
        .join(format!("{run_id}.json"))
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use psionic_train::{
        RemoteTrainingRunIndex, RemoteTrainingRunIndexEntry, build_remote_training_run_index,
        sample_google_summary_only_visualization_bundle,
        sample_parameter_golf_live_visualization_bundle,
    };
    use tempfile::tempdir;

    #[test]
    fn source_sync_persists_index_and_bundles_into_cache() {
        let temp = tempdir().expect("tempdir");
        let source_root = temp.path().join("source");
        let cache_root = temp.path().join("cache");
        write_fixture_source(&source_root);

        let loaded = load_remote_training_from_source(
            Some(source_root.as_path()),
            None,
            cache_root.as_path(),
        )
        .expect("load live source");

        assert!(!loaded.using_cached_mirror);
        assert_eq!(loaded.run_index.entries.len(), 2);
        assert_eq!(loaded.bundles.len(), 2);
        assert!(
            cache_root
                .join(REMOTE_TRAINING_INDEX_RELATIVE_PATH)
                .exists()
        );
        assert!(
            cache_bundle_path(cache_root.as_path(), "psion-google-summary-only-sample").exists()
        );
        assert!(
            cache_bundle_path(
                cache_root.as_path(),
                "parameter-golf-runpod-single-h100-live-sample"
            )
            .exists()
        );
    }

    #[test]
    fn cache_load_rehydrates_when_live_source_is_missing() {
        let temp = tempdir().expect("tempdir");
        let source_root = temp.path().join("source");
        let cache_root = temp.path().join("cache");
        write_fixture_source(&source_root);
        load_remote_training_from_source(Some(source_root.as_path()), None, cache_root.as_path())
            .expect("prime cache");
        fs::remove_dir_all(&source_root).expect("remove live source");

        let loaded = load_remote_training_live_or_cached(
            Some(source_root.as_path()),
            None,
            cache_root.as_path(),
        )
        .expect("load cached mirror");

        assert!(loaded.using_cached_mirror);
        assert_eq!(loaded.run_index.entries.len(), 2);
        assert_eq!(loaded.bundles.len(), 2);
    }

    #[test]
    fn refresh_interval_drops_to_one_second_when_any_run_is_active() {
        let parameter_golf =
            sample_parameter_golf_live_visualization_bundle().expect("parameter golf sample");
        let google = sample_google_summary_only_visualization_bundle().expect("google sample");
        let index = build_remote_training_run_index(RemoteTrainingRunIndex {
            schema_version: String::new(),
            index_id: "idx".to_string(),
            generated_at_ms: 1,
            entries: vec![
                RemoteTrainingRunIndexEntry {
                    provider: google.provider,
                    profile_id: google.profile_id.clone(),
                    lane_id: google.lane_id.clone(),
                    run_id: google.run_id.clone(),
                    repo_revision: google.repo_revision.clone(),
                    result_classification: google.result_classification,
                    series_status: google.series_status,
                    series_unavailable_reason: google.series_unavailable_reason.clone(),
                    last_heartbeat_at_ms: google.refresh_contract.last_heartbeat_at_ms,
                    bundle_artifact_uri: None,
                    bundle_digest: Some(google.bundle_digest.clone()),
                    summary_label: "google".to_string(),
                    detail: "google".to_string(),
                },
                RemoteTrainingRunIndexEntry {
                    provider: parameter_golf.provider,
                    profile_id: parameter_golf.profile_id.clone(),
                    lane_id: parameter_golf.lane_id.clone(),
                    run_id: parameter_golf.run_id.clone(),
                    repo_revision: parameter_golf.repo_revision.clone(),
                    result_classification: parameter_golf.result_classification,
                    series_status: parameter_golf.series_status,
                    series_unavailable_reason: parameter_golf.series_unavailable_reason.clone(),
                    last_heartbeat_at_ms: parameter_golf.refresh_contract.last_heartbeat_at_ms,
                    bundle_artifact_uri: None,
                    bundle_digest: Some(parameter_golf.bundle_digest.clone()),
                    summary_label: "pgolf".to_string(),
                    detail: "pgolf".to_string(),
                },
            ],
            detail: "idx".to_string(),
            index_digest: String::new(),
        })
        .expect("index");
        let bundles = BTreeMap::from([
            (google.run_id.clone(), google),
            (parameter_golf.run_id.clone(), parameter_golf),
        ]);

        assert_eq!(
            refresh_interval_for_runs(index.entries.as_slice(), &bundles),
            REMOTE_TRAINING_ACTIVE_REFRESH_INTERVAL_MS
        );
    }

    fn write_fixture_source(source_root: &Path) {
        let google = sample_google_summary_only_visualization_bundle().expect("google sample");
        let parameter_golf =
            sample_parameter_golf_live_visualization_bundle().expect("parameter golf sample");
        let index = sample_index(&google, &parameter_golf);
        let fixture_dir = source_root.join("fixtures").join("training_visualization");
        fs::create_dir_all(&fixture_dir).expect("create fixture dir");
        fs::write(
            fixture_dir
                .join("psion_google_summary_only_remote_training_visualization_bundle_v1.json"),
            serde_json::to_vec_pretty(&google).expect("encode google"),
        )
        .expect("write google");
        fs::write(
            fixture_dir.join("parameter_golf_live_remote_training_visualization_bundle_v1.json"),
            serde_json::to_vec_pretty(&parameter_golf).expect("encode pgolf"),
        )
        .expect("write pgolf");
        fs::write(
            fixture_dir.join("remote_training_run_index_v1.json"),
            serde_json::to_vec_pretty(&index).expect("encode index"),
        )
        .expect("write index");
    }

    fn sample_index(
        google: &RemoteTrainingVisualizationBundle,
        parameter_golf: &RemoteTrainingVisualizationBundle,
    ) -> RemoteTrainingRunIndex {
        build_remote_training_run_index(RemoteTrainingRunIndex {
            schema_version: String::new(),
            index_id: "remote-training-run-index-sample-v1".to_string(),
            generated_at_ms: 1,
            entries: vec![
                RemoteTrainingRunIndexEntry {
                    provider: google.provider,
                    profile_id: google.profile_id.clone(),
                    lane_id: google.lane_id.clone(),
                    run_id: google.run_id.clone(),
                    repo_revision: google.repo_revision.clone(),
                    result_classification: google.result_classification,
                    series_status: google.series_status,
                    series_unavailable_reason: google.series_unavailable_reason.clone(),
                    last_heartbeat_at_ms: google.refresh_contract.last_heartbeat_at_ms,
                    bundle_artifact_uri: Some("fixtures/training_visualization/psion_google_summary_only_remote_training_visualization_bundle_v1.json".to_string()),
                    bundle_digest: Some(google.bundle_digest.clone()),
                    summary_label: "Google single-node Psion summary-only sample".to_string(),
                    detail: "Google summary-only".to_string(),
                },
                RemoteTrainingRunIndexEntry {
                    provider: parameter_golf.provider,
                    profile_id: parameter_golf.profile_id.clone(),
                    lane_id: parameter_golf.lane_id.clone(),
                    run_id: parameter_golf.run_id.clone(),
                    repo_revision: parameter_golf.repo_revision.clone(),
                    result_classification: parameter_golf.result_classification,
                    series_status: parameter_golf.series_status,
                    series_unavailable_reason: parameter_golf.series_unavailable_reason.clone(),
                    last_heartbeat_at_ms: parameter_golf.refresh_contract.last_heartbeat_at_ms,
                    bundle_artifact_uri: Some("fixtures/training_visualization/parameter_golf_live_remote_training_visualization_bundle_v1.json".to_string()),
                    bundle_digest: Some(parameter_golf.bundle_digest.clone()),
                    summary_label: "RunPod single-H100 PGOLF live sample".to_string(),
                    detail: "RunPod live".to_string(),
                },
            ],
            detail: "idx".to_string(),
            index_digest: String::new(),
        })
        .expect("index")
    }
}
