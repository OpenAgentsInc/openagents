use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use psionic_train::{XtrainExplorerIndex, XtrainExplorerIndexEntry, XtrainExplorerSnapshot};

use crate::app_state::{PaneLoadState, PaneStatusAccess, RenderState};

const XTRAIN_EXPLORER_INDEX_RELATIVE_PATH: &str = "fixtures/training/xtrain_explorer_index_v1.json";
const XTRAIN_EXPLORER_REFRESH_INTERVAL_MS: u64 = 15_000;

pub const OPENAGENTS_XTRAIN_EXPLORER_SOURCE_ROOT_ENV: &str =
    "OPENAGENTS_XTRAIN_EXPLORER_SOURCE_ROOT";
pub const OPENAGENTS_XTRAIN_EXPLORER_INDEX_PATH_ENV: &str =
    "OPENAGENTS_XTRAIN_EXPLORER_INDEX_PATH";

struct LoadedXtrainExplorer {
    source_root: Option<PathBuf>,
    index_path: PathBuf,
    snapshot_path: PathBuf,
    index: XtrainExplorerIndex,
    snapshot: XtrainExplorerSnapshot,
}

pub(crate) fn default_xtrain_explorer_source_root_hint() -> Option<PathBuf> {
    std::env::var(OPENAGENTS_XTRAIN_EXPLORER_SOURCE_ROOT_ENV)
        .ok()
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .or_else(search_workspace_psionic_root)
}

pub(crate) fn default_xtrain_explorer_index_path_hint() -> Option<PathBuf> {
    std::env::var(OPENAGENTS_XTRAIN_EXPLORER_INDEX_PATH_ENV)
        .ok()
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| {
            default_xtrain_explorer_source_root_hint()
                .map(|root| root.join(XTRAIN_EXPLORER_INDEX_RELATIVE_PATH))
                .filter(|path| path.exists())
        })
}

pub(crate) fn refresh_xtrain_explorer_state(state: &mut RenderState, force: bool) -> bool {
    let now_epoch_ms = current_epoch_ms();
    let pane = &state.xtrain_explorer;
    let due = force
        || pane.last_refreshed_at_epoch_ms.is_none_or(|last| {
            now_epoch_ms.saturating_sub(last) >= XTRAIN_EXPLORER_REFRESH_INTERVAL_MS
        });
    if !due {
        return false;
    }

    let previous_load_state = pane.load_state;
    let previous_last_error = pane.last_error.clone();
    let previous_last_action = pane.last_action.clone();
    let previous_source_root = pane.source_root.clone();
    let previous_index_path = pane.index_path.clone();
    let previous_snapshot_path = pane.snapshot_path.clone();
    let previous_selected_snapshot_id = pane.selected_snapshot_id.clone();
    let previous_selected_participant_id = pane.selected_participant_id.clone();
    let previous_index = pane.index.clone();
    let previous_snapshot = pane.snapshot.clone();
    let selected_snapshot_id = pane.selected_snapshot_id.clone();

    let result = load_xtrain_explorer(
        pane.source_root_hint.as_deref(),
        pane.index_path_hint.as_deref(),
        selected_snapshot_id.as_deref(),
    );

    let pane = &mut state.xtrain_explorer;
    let mut changed = false;
    match result {
        Ok(loaded) => {
            let selected_participant_id = next_selected_participant_id(
                loaded.snapshot.participants.as_slice(),
                pane.selected_participant_id.as_deref(),
            );
            changed |= previous_source_root != loaded.source_root;
            changed |= previous_index_path.as_ref() != Some(&loaded.index_path);
            changed |= previous_snapshot_path.as_ref() != Some(&loaded.snapshot_path);
            changed |= previous_selected_snapshot_id.as_deref()
                != Some(loaded.snapshot.snapshot_id.as_str());
            changed |= previous_selected_participant_id != selected_participant_id;
            changed |= previous_index.as_ref() != Some(&loaded.index);
            changed |= previous_snapshot.as_ref() != Some(&loaded.snapshot);
            pane.source_root = loaded.source_root;
            pane.index_path = Some(loaded.index_path);
            pane.snapshot_path = Some(loaded.snapshot_path);
            pane.selected_snapshot_id = Some(loaded.snapshot.snapshot_id.clone());
            pane.selected_participant_id = selected_participant_id;
            pane.index = Some(loaded.index);
            pane.snapshot = Some(loaded.snapshot);
            pane.pane_set_ready(format!(
                "Loaded XTRAIN explorer snapshot {} with {} participants and {} windows",
                pane.selected_snapshot_id.as_deref().unwrap_or("unknown"),
                pane.snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.participants.len())
                    .unwrap_or(0),
                pane.snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.windows.len())
                    .unwrap_or(0)
            ));
        }
        Err(error) => {
            pane.load_state = PaneLoadState::Error;
            pane.last_action = Some("XTRAIN explorer refresh failed".to_string());
            pane.last_error = Some(error);
        }
    }

    changed |= previous_load_state != pane.load_state;
    changed |= previous_last_error != pane.last_error;
    changed |= previous_last_action != pane.last_action;
    changed |= pane.last_refreshed_at_epoch_ms != Some(now_epoch_ms);
    pane.last_refreshed_at_epoch_ms = Some(now_epoch_ms);
    changed
}

fn load_xtrain_explorer(
    source_root_hint: Option<&Path>,
    index_path_hint: Option<&Path>,
    selected_snapshot_id: Option<&str>,
) -> Result<LoadedXtrainExplorer, String> {
    let source_root = source_root_hint
        .map(PathBuf::from)
        .or_else(default_xtrain_explorer_source_root_hint);
    let index_path = index_path_hint
        .map(PathBuf::from)
        .or_else(|| {
            source_root
                .as_ref()
                .map(|root| root.join(XTRAIN_EXPLORER_INDEX_RELATIVE_PATH))
        })
        .or_else(default_xtrain_explorer_index_path_hint)
        .ok_or_else(|| {
            "XTRAIN explorer source is not configured; set OPENAGENTS_XTRAIN_EXPLORER_SOURCE_ROOT or OPENAGENTS_XTRAIN_EXPLORER_INDEX_PATH".to_string()
        })?;
    let raw = fs::read_to_string(&index_path).map_err(|error| {
        format!(
            "failed to read XTRAIN explorer index {}: {error}",
            index_path.display()
        )
    })?;
    let index = serde_json::from_str::<XtrainExplorerIndex>(raw.as_str()).map_err(|error| {
        format!(
            "failed to decode XTRAIN explorer index {}: {error}",
            index_path.display()
        )
    })?;
    index.validate().map_err(|error| {
        format!(
            "XTRAIN explorer index {} failed validation: {error}",
            index_path.display()
        )
    })?;

    let entry =
        selected_snapshot_entry(&index, selected_snapshot_id).ok_or_else(|| {
            format!("XTRAIN explorer index {} has no entries", index_path.display())
        })?;
    let snapshot_path = resolve_snapshot_path(
        source_root.as_deref(),
        index_path.as_path(),
        entry.snapshot_artifact_uri.as_deref(),
    )
    .ok_or_else(|| {
        format!(
            "XTRAIN explorer index {} does not expose a usable snapshot_artifact_uri",
            index_path.display()
        )
    })?;
    let snapshot_raw = fs::read_to_string(&snapshot_path).map_err(|error| {
        format!(
            "failed to read XTRAIN explorer snapshot {}: {error}",
            snapshot_path.display()
        )
    })?;
    let snapshot =
        serde_json::from_str::<XtrainExplorerSnapshot>(snapshot_raw.as_str()).map_err(|error| {
            format!(
                "failed to decode XTRAIN explorer snapshot {}: {error}",
                snapshot_path.display()
            )
        })?;
    snapshot.validate().map_err(|error| {
        format!(
            "XTRAIN explorer snapshot {} failed validation: {error}",
            snapshot_path.display()
        )
    })?;

    if snapshot.snapshot_id != entry.snapshot_id {
        return Err(format!(
            "XTRAIN explorer snapshot {} does not match selected index entry {}",
            snapshot.snapshot_id, entry.snapshot_id
        ));
    }
    if entry
        .snapshot_digest
        .as_deref()
        .is_some_and(|digest| digest != snapshot.snapshot_digest)
    {
        return Err(format!(
            "XTRAIN explorer snapshot digest mismatch for {}",
            snapshot.snapshot_id
        ));
    }

    Ok(LoadedXtrainExplorer {
        source_root,
        index_path,
        snapshot_path,
        index,
        snapshot,
    })
}

fn selected_snapshot_entry<'a>(
    index: &'a XtrainExplorerIndex,
    selected_snapshot_id: Option<&str>,
) -> Option<&'a XtrainExplorerIndexEntry> {
    selected_snapshot_id
        .and_then(|selected| {
            index
                .entries
                .iter()
                .find(|entry| entry.snapshot_id == selected)
        })
        .or_else(|| index.entries.iter().max_by_key(|entry| entry.generated_at_ms))
}

fn next_selected_participant_id(
    participants: &[psionic_train::XtrainExplorerParticipantNode],
    selected_participant_id: Option<&str>,
) -> Option<String> {
    selected_participant_id
        .and_then(|selected| {
            participants
                .iter()
                .find(|participant| participant.participant_id == selected)
                .map(|participant| participant.participant_id.clone())
        })
        .or_else(|| participants.first().map(|participant| participant.participant_id.clone()))
}

fn resolve_snapshot_path(
    source_root: Option<&Path>,
    index_path: &Path,
    artifact_uri: Option<&str>,
) -> Option<PathBuf> {
    let artifact_uri = artifact_uri?.trim();
    if artifact_uri.is_empty() {
        return None;
    }
    let candidate = PathBuf::from(artifact_uri);
    if candidate.is_absolute() {
        return Some(candidate);
    }
    source_root
        .map(|root| root.join(&candidate))
        .or_else(|| index_path.parent().map(|parent| parent.join(&candidate)))
}

fn search_workspace_psionic_root() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    [
        manifest_dir.join("../../../psionic"),
        manifest_dir.join("../../../../psionic"),
    ]
    .into_iter()
    .find(|candidate| candidate.exists())
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}
