//! The OpenAgents gateway's models in /models (#11526).
//!
//! When the Coder loop can reach the gateway as the signed-in account,
//! /models lists the gateway's own models (its `/v1/models`), kept on disk
//! with when they were fetched and refreshed in the background, so the
//! picker opens at once. A pick sends the Coder loop's turns to that model
//! through the gateway; `auto` stays the default. Nothing is listed when
//! the account can't reach the gateway.
//!
//! The cache is one small JSON file beside Coder's other settings,
//! `gateway-models.json`, keyed by the gateway and account it was read as,
//! so another sign-in never sees it. The pick is `gateway-model.json`.

use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::Model;

/// The picker rows' plugin name, so /models knows a gateway row.
pub const PICKER: &str = "openagents-gateway";

/// What the picker calls the gateway's models.
const PROVIDER: &str = "OpenAgents";

const CACHE_FILE: &str = "gateway-models.json";
const CHOICE_FILE: &str = "gateway-model.json";

/// How long a fetched list is used before a background refresh.
pub const FRESH_FOR: u64 = 15 * 60;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const BODY_LIMIT: usize = 512 * 1024;

/// One model the gateway offers, as the cache keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub id: String,
    #[serde(default)]
    pub context: Option<u32>,
    #[serde(default)]
    pub max_output: Option<u32>,
    #[serde(default)]
    pub free: bool,
    #[serde(default)]
    pub images: bool,
}

#[derive(Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    origin: String,
    account: String,
    /// Unix seconds.
    fetched_at: u64,
    models: Vec<Row>,
}

#[derive(Serialize, Deserialize)]
struct ChoiceFile {
    version: u32,
    model: String,
}

/// A finished background refresh, for the open picker to take.
static FRESH: Mutex<Option<(String, Vec<Row>)>> = Mutex::new(None);
/// The account a refresh is running for, so two never overlap.
static RUNNING: Mutex<Option<String>> = Mutex::new(None);

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// The models a gateway `/v1/models` body offers the Coder loop: concrete
/// models that take tools, without the gateway's own routers (`auto` is
/// already the default).
#[must_use]
pub fn parse(body: &Value) -> Vec<Row> {
    let Some(data) = body.get("data").and_then(Value::as_array) else {
        return Vec::new();
    };
    data.iter()
        .filter_map(|entry| {
            let id = entry.get("id")?.as_str()?.trim();
            let meta = entry.get("openagents");
            if id.is_empty()
                || meta
                    .and_then(|meta| meta.get("router"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            {
                return None;
            }
            let providers = meta
                .and_then(|meta| meta.get("providers"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let capable = |name: &str| {
                providers
                    .iter()
                    .any(|provider| provider["capabilities"][name].as_bool().unwrap_or(false))
            };
            if !capable("tools") {
                return None;
            }
            let largest = |field: &str| {
                providers
                    .iter()
                    .filter_map(|provider| provider[field].as_u64())
                    .max()
                    .and_then(|value| u32::try_from(value).ok())
            };
            Some(Row {
                id: id.to_owned(),
                context: largest("context"),
                max_output: largest("max_output"),
                free: meta
                    .and_then(|meta| meta.get("free"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                images: capable("images"),
            })
        })
        .collect()
}

pub fn cached_in(root: &Path, account: &openagents_login::Saved) -> Option<(u64, Vec<Row>)> {
    let path = root.join(CACHE_FILE);
    let file: CacheFile = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    (file.version == 1 && file.origin == account.origin && file.account == account.account)
        .then_some((file.fetched_at, file.models))
}

fn keep_in(root: &Path, account: &openagents_login::Saved, models: &[Row], at: u64) {
    let file = CacheFile {
        version: 1,
        origin: account.origin.clone(),
        account: account.account.clone(),
        fetched_at: at,
        models: models.to_vec(),
    };
    if let Ok(bytes) = serde_json::to_vec_pretty(&file) {
        let _ = std::fs::create_dir_all(root);
        let _ = std::fs::write(root.join(CACHE_FILE), bytes);
    }
}

/// Forgets the kept list: the account can't reach the gateway.
fn forget(root: &Path) {
    let _ = std::fs::remove_file(root.join(CACHE_FILE));
}

/// Reads the gateway's models as `account`.
///
/// # Errors
/// `Err(true)` when the gateway refused the sign-in, `Err(false)` when it
/// couldn't be reached or answered something else.
pub async fn fetch(account: &openagents_login::Saved) -> Result<Vec<Row>, bool> {
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| false)?;
    let response = client
        .get(format!(
            "{}/api/v1/models",
            account.origin.trim_end_matches('/')
        ))
        .bearer_auth(account.token())
        .send()
        .await
        .map_err(|_| false)?;
    let status = response.status().as_u16();
    if matches!(status, 401 | 403) {
        return Err(true);
    }
    if !response.status().is_success() {
        return Err(false);
    }
    let bytes = response.bytes().await.map_err(|_| false)?;
    if bytes.len() > BODY_LIMIT {
        return Err(false);
    }
    let body: Value = serde_json::from_slice(&bytes).map_err(|_| false)?;
    Ok(parse(&body))
}

/// Fetches and keeps the list for `account`: a refused sign-in forgets
/// it; a failed fetch keeps what was there.
fn refresh_now(root: &Path, account: &openagents_login::Saved) -> Option<Vec<Row>> {
    let account = account.clone();
    let fetched = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .ok()?;
        let result = runtime.block_on(fetch(&account));
        Some((account, result))
    })
    .join()
    .ok()
    .flatten()?;
    match fetched {
        (account, Ok(models)) => {
            keep_in(root, &account, &models, now());
            Some(models)
        }
        (_, Err(true)) => {
            forget(root);
            Some(Vec::new())
        }
        (_, Err(false)) => None,
    }
}

/// The gateway's models for `account`, when it can reach the gateway: the
/// list kept under `root` (Coder's settings folder), refreshed first when
/// it is older than [`FRESH_FOR`] (for headless callers, which can wait).
#[must_use]
pub fn listed_now(root: &Path, account: Option<&openagents_login::Saved>) -> Vec<Row> {
    let Some(account) = account.filter(|account| reachable(account)) else {
        return Vec::new();
    };
    match cached_in(root, account) {
        Some((at, models)) if now().saturating_sub(at) < FRESH_FOR => models,
        kept => refresh_now(root, account)
            .or_else(|| kept.map(|(_, models)| models))
            .unwrap_or_default(),
    }
}

/// The kept list for `account` without waiting, and a background refresh
/// when it is stale; [`take_fresh`] hands the refreshed list over.
#[must_use]
pub fn listed(root: &Path, account: Option<&openagents_login::Saved>) -> Vec<Row> {
    let Some(account) = account.filter(|account| reachable(account)) else {
        return Vec::new();
    };
    let kept = cached_in(root, account);
    if kept
        .as_ref()
        .is_none_or(|(at, _)| now().saturating_sub(*at) >= FRESH_FOR)
    {
        refresh_in_background(root, account);
    }
    kept.map(|(_, models)| models).unwrap_or_default()
}

fn refresh_in_background(root: &Path, account: &openagents_login::Saved) {
    {
        let mut running = RUNNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if running.as_deref() == Some(account.account.as_str()) {
            return;
        }
        *running = Some(account.account.clone());
    }
    let account = account.clone();
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        let models = refresh_now(&root, &account);
        *RUNNING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        if let Some(models) = models {
            *FRESH
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some((account.account, models));
        }
    });
}

/// A finished background refresh, once: the account it was read as and
/// its list.
#[must_use]
pub fn take_fresh() -> Option<(String, Vec<Row>)> {
    FRESH
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
}

/// Whether the Coder loop can ask the gateway as `account` now.
#[must_use]
pub fn reachable(account: &openagents_login::Saved) -> bool {
    crate::provider::gateway_open(account)
}

/// The picker rows for `models`: `auto` first when `with_auto` (no other
/// list in the picker already offers it), then each gateway model.
#[must_use]
pub fn catalog(models: &[Row], with_auto: bool) -> Vec<Model> {
    let auto = with_auto.then(|| Model {
        plugin: PICKER.into(),
        provider: PROVIDER.into(),
        id: crate::models::AUTO.into(),
        name: "Auto".into(),
        description: "OpenAgents picks the model".into(),
        context_length: None,
        max_output_tokens: None,
        efforts: Vec::new(),
        default_effort: None,
        supports_output_limit: false,
    });
    auto.into_iter()
        .chain(models.iter().map(|row| Model {
            plugin: PICKER.into(),
            provider: PROVIDER.into(),
            id: row.id.clone(),
            name: crate::ui::rail_model_name(&row.id),
            description: if row.free {
                "On OpenAgents, free allowance".into()
            } else {
                "On OpenAgents, from your credit".into()
            },
            context_length: row.context,
            max_output_tokens: row.max_output,
            efforts: Vec::new(),
            default_effort: None,
            supports_output_limit: false,
        }))
        .collect()
}

/// The pick the Coder loop's next turn runs: `picked` while `account`
/// can reach the gateway, else `None` (`auto`).
#[must_use]
pub fn active(picked: Option<&str>, account: Option<&openagents_login::Saved>) -> Option<String> {
    account
        .filter(|account| reachable(account))
        .and(picked)
        .map(str::to_owned)
}

/// The pick saved under `root`, Coder's settings folder, or `None` for
/// `auto`.
#[must_use]
pub fn load_choice(root: &Path) -> Option<String> {
    std::fs::read(root.join(CHOICE_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<ChoiceFile>(&bytes).ok())
        .filter(|file| file.version == 1 && crate::models::pinned(&file.model))
        .map(|file| file.model)
}

/// Saves `model` as the pick under `root` (`None` or `auto` for `auto`)
/// and returns what the pick now is.
///
/// # Errors
/// The pick could not be saved; the earlier one stays.
pub fn choose(root: Option<&Path>, model: Option<&str>) -> Result<Option<String>, String> {
    let model = model.filter(|model| crate::models::pinned(model));
    if let Some(root) = root {
        let path = root.join(CHOICE_FILE);
        let saved = match model {
            Some(model) => serde_json::to_vec_pretty(&ChoiceFile {
                version: 1,
                model: model.into(),
            })
            .map_err(|error| error.to_string())
            .and_then(|bytes| {
                std::fs::create_dir_all(root)
                    .and_then(|()| std::fs::write(&path, bytes))
                    .map_err(|error| error.to_string())
            }),
            None => match std::fs::remove_file(&path) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    Err(error.to_string())
                }
                _ => Ok(()),
            },
        };
        saved.map_err(|_| "Cannot save the model. The earlier one stays.".to_owned())?;
    }
    Ok(model.map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> Value {
        serde_json::json!({"object":"list","data":[
            {"id":"google/gemini-2.5-flash-lite","openagents":{"free":true,"providers":[
                {"context":1048576,"max_output":65536,"capabilities":{"tools":true,"images":true}}]}},
            {"id":"zai/glm-5.3-flash","openagents":{"providers":[
                {"context":1000000,"max_output":128000,"capabilities":{"tools":true,"images":false}}]}},
            {"id":"embed/no-tools","openagents":{"providers":[{"capabilities":{"tools":false}}]}},
            {"id":"openagents/auto","openagents":{"router":true}},
            {"id":"openagents/code","openagents":{"router":true,"models":["zai/glm-5.3-flash"]}}
        ]})
    }

    #[test]
    fn the_list_is_the_gateways_tool_models_without_its_routers() {
        let rows = parse(&body());
        let ids: Vec<_> = rows.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(ids, ["google/gemini-2.5-flash-lite", "zai/glm-5.3-flash"]);
        assert!(rows[0].free && rows[0].images && !rows[1].free);
        assert_eq!(rows[1].max_output, Some(128_000));
        assert!(parse(&serde_json::json!({"error":"nope"})).is_empty());
        let rows = catalog(&rows, true);
        assert_eq!(rows[0].id, crate::models::AUTO);
        assert_eq!(rows[1].name, "Gemini 2.5 Flash Lite");
        assert_eq!(rows[2].name, "GLM 5.3 Flash");
        assert!(rows.iter().all(|row| row.plugin == PICKER));
        assert_eq!(catalog(&[], false).len(), 0);
    }

    fn account(name: &str) -> openagents_login::Saved {
        serde_json::from_value(serde_json::json!({
            "origin": "http://fixture", "account": name, "label": name,
            "expires_at": u64::MAX, "token": "fixture-session-token",
        }))
        .unwrap()
    }

    /// The kept list says when it was fetched and belongs to one sign-in.
    #[test]
    fn the_kept_list_is_one_accounts_with_its_fetch_time() {
        let dir = tempfile::tempdir().unwrap();
        let rows = parse(&body());
        keep_in(dir.path(), &account("acct_a"), &rows, 1_700_000_000);
        assert_eq!(
            cached_in(dir.path(), &account("acct_a")),
            Some((1_700_000_000, rows))
        );
        assert_eq!(cached_in(dir.path(), &account("acct_b")), None);
        let text = std::fs::read_to_string(dir.path().join(CACHE_FILE)).unwrap();
        assert!(!text.contains("fixture-session-token"));
    }

    #[test]
    fn the_pick_is_saved_and_auto_clears_it() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_choice(dir.path()), None);
        let picked = choose(Some(dir.path()), Some("zai/glm-5.3-flash")).unwrap();
        assert_eq!(picked.as_deref(), Some("zai/glm-5.3-flash"));
        assert_eq!(load_choice(dir.path()), picked);
        assert_eq!(choose(Some(dir.path()), Some("auto")).unwrap(), None);
        assert_eq!(load_choice(dir.path()), None);
        // Signed out, a pick waits and the turn is auto.
        assert_eq!(active(picked.as_deref(), None), None);
        assert_eq!(
            active(picked.as_deref(), Some(&account("acct_pick"))),
            picked
        );
    }
}
