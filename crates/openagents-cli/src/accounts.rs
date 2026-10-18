//! Read account quota through the running app engine's shared usage cache.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use openagents_proto::{AgentAccount, AgentAccountsSnapshot, HarnessId};
use openagents_rpc::{RpcClient, methods};
use serde_json::{Value, json};

use crate::{Output, argv};

pub(crate) const USAGE: &str = "usage: openagents accounts COMMAND [OPTIONS]
  usage --device HOST [--harness codex] [--account active|ID] [--refresh] [--ipc-port PORT]
        Read Codex account quota from the running OpenAgents app engine.
        HOST is local or an exact device UUID; account defaults to active.
        Use --json for one JSON document. Cached reads never request a quota
        refresh. --refresh asks the engine to refresh all accounts on HOST,
        honoring its 30-second minimum interval and provider retry delays.
        A refresh request can return the same cached observation.
--ipc-port selects the local app engine (default OPENAGENTS_IPC_PORT or the
desktop port). Remote HOST requests go through that engine's paired connection.
Fresh means fetched within five minutes, without a usage error or an elapsed
quota reset. Stale data remains usable; missing data never means zero usage.
Exit 0 means a selected account was read, including stale or unavailable usage.
Exit 1 means host/account selection or communication failed; 64 means invalid
arguments. Quota is separate from chat context tokens and the capacity book.
See docs/coder/guides/account-usage.md for JSON fields and timestamp units.";

#[cfg(test)]
pub(crate) const EFFECTS: &[coder::cli_route::tree::Declared] =
    &[coder::cli_route::tree::Declared::computer(
        "usage",
        coder::cli_route::tree::Effect::ReadOnly,
    )];

const FRESH_FOR_MS: i64 = 300_000;
const RPC_TIMEOUT: Duration = Duration::from_secs(60);

struct Options {
    device: String,
    account: String,
    refresh: bool,
    port: Option<u16>,
}

impl Options {
    fn parse(words: &[String]) -> Result<Self, String> {
        let args = argv::parse_command(
            words,
            "accounts usage",
            &["device", "harness", "account", "ipc-port"],
            &["refresh"],
            0,
            0,
        )?;
        let device = args
            .option("device")
            .ok_or("give --device local or a device UUID")?;
        if device != "local" && uuid::Uuid::parse_str(device).is_err() {
            return Err("--device must be local or an exact device UUID".into());
        }
        if args.option("harness").unwrap_or("codex") != "codex" {
            return Err("accounts usage currently supports --harness codex".into());
        }
        let account = args.option("account").unwrap_or("active");
        if account.is_empty() {
            return Err("--account needs active or an account ID".into());
        }
        let port = args.option("ipc-port").map(parse_port).transpose()?;
        Ok(Self {
            device: if device == "local" {
                device.into()
            } else {
                uuid::Uuid::parse_str(device)
                    .expect("validated UUID")
                    .to_string()
            },
            account: account.into(),
            refresh: args.switch("refresh"),
            port,
        })
    }
}

fn parse_port(value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| "--ipc-port or OPENAGENTS_IPC_PORT needs a port from 1 to 65535".into())
}

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|word| matches!(word.as_str(), "help" | "--help" | "-h"))
    {
        println!("{USAGE}");
        return 0;
    }
    let Some((command, rest)) = words.split_first() else {
        return output.usage("accounts", "choose usage", USAGE);
    };
    if command != "usage" {
        return output.usage("accounts", "choose usage", USAGE);
    }
    let options = match Options::parse(rest) {
        Ok(options) => options,
        Err(error) => return output.usage("accounts", &error, USAGE),
    };
    let port = match options.port.map(Ok).unwrap_or_else(|| {
        openagents_brand::env_var("OPENAGENTS_IPC_PORT")
            .map_or(Ok(openagents_brand::DEFAULT_IPC_PORT), |port| {
                parse_port(&port)
            })
    }) {
        Ok(port) => port,
        Err(error) => return output.usage("accounts", &error, USAGE),
    };
    let result = crate::runtime().block_on(async {
        let client = tokio::time::timeout(
            RPC_TIMEOUT,
            openagents_rpc::connect_ws(&format!("ws://127.0.0.1:{port}")),
        ).await.map_err(|_| "The OpenAgents app engine did not answer")?
            .map_err(|_| "Couldn't connect to the OpenAgents app engine; check that it is running and --ipc-port is correct")?;
        query(&client, &options, now).await
    });
    match result {
        Ok(value) => {
            output.emit(&value, render);
            0
        }
        Err(error) => output.fail("accounts", error),
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

async fn call(client: &RpcClient, method: &str, params: Value) -> Result<Value, &'static str> {
    tokio::time::timeout(RPC_TIMEOUT, client.call(method, params))
        .await
        .map_err(|_| "The selected device did not answer in time")?
        .map_err(|_| "Couldn't read accounts from the selected device")
}

async fn snapshot(
    client: &RpcClient,
    device: &str,
    refresh: bool,
) -> Result<AgentAccountsSnapshot, &'static str> {
    let value = call(
        client,
        methods::LIST_AGENT_ACCOUNTS,
        json!({"targetDeviceId": device, "forceUsage": refresh}),
    )
    .await?;
    serde_json::from_value(value)
        .map_err(|_| "The selected device returned an unreadable accounts response")
}

fn select<'a>(
    snapshot: &'a AgentAccountsSnapshot,
    selector: &str,
) -> Result<&'a AgentAccount, &'static str> {
    let mut accounts = snapshot.accounts.iter().filter(|account| {
        account.harness == HarnessId::Codex
            && if selector == "active" {
                account.active
            } else {
                account.id == selector
            }
    });
    let account = accounts
        .next()
        .ok_or("No matching Codex account on the selected device; check --account")?;
    if accounts.next().is_some() {
        return Err("More than one Codex account matches; give an exact --account ID");
    }
    Ok(account)
}

async fn query(
    client: &RpcClient,
    options: &Options,
    clock: impl Fn() -> i64,
) -> Result<Value, &'static str> {
    let device = if options.device == "local" {
        call(client, methods::LOCAL_DEVICE, json!({}))
            .await?
            .get("deviceId")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or("The app engine did not identify its local device")?
            .to_owned()
    } else {
        options.device.clone()
    };
    let before = snapshot(client, &device, false).await?;
    // Resolve first, so a refresh never changes which account `active` chose.
    let mut account = select(&before, &options.account)?.clone();
    let mut refresh_error = None;
    let mut updated = false;
    if options.refresh {
        match snapshot(client, &device, true).await {
            Ok(after) => {
                let next = select(&after, &account.id)?;
                updated = next
                    .usage_fetched_at
                    .is_some_and(|at| account.usage_fetched_at.is_none_or(|old| at > old));
                account = next.clone();
            }
            Err(error) => refresh_error = Some(error),
        }
    }
    Ok(report(
        &device,
        &account,
        clock(),
        options.refresh,
        updated,
        refresh_error,
    ))
}

fn report(
    device: &str,
    account: &AgentAccount,
    observed_at: i64,
    requested: bool,
    updated: bool,
    refresh_error: Option<&str>,
) -> Value {
    let age = account
        .usage_fetched_at
        .and_then(|at| observed_at.checked_sub(at))
        .filter(|age| *age >= 0);
    let available = !account.usage_windows.is_empty();
    let reset_elapsed = account.usage_windows.iter().any(|window| {
        window
            .resets_at
            .is_some_and(|at| at.timestamp_millis() <= observed_at)
    });
    let state = if !available {
        if account.usage_fetched_at.is_none() && account.usage_error.is_none() {
            "neverFetched"
        } else {
            "unavailable"
        }
    } else if age.is_some_and(|age| age <= FRESH_FOR_MS)
        && account.usage_error.is_none()
        && refresh_error.is_none()
        && !reset_elapsed
    {
        "fresh"
    } else {
        "stale"
    };
    let windows: Vec<_> = account
        .usage_windows
        .iter()
        .map(|window| {
            json!({
                "label": window.label,
                "usedFraction": window.used_fraction,
                "usedPercent": f64::from(window.used_fraction) * 100.0,
                "resetsAt": window.resets_at,
            })
        })
        .collect();
    json!({
        "schema": "openagents.accounts.usage.v1",
        "deviceId": device,
        "account": {"id": account.id, "harness": account.harness, "active": account.active, "planLabel": account.plan_label},
        "windows": windows,
        "usageFetchedAt": account.usage_fetched_at,
        "cachedAt": account.usage_fetched_at,
        "observedAt": observed_at,
        "ageMs": age,
        "freshForMs": FRESH_FOR_MS,
        "state": state,
        "stale": state != "fresh",
        "available": available,
        "usageError": account.usage_error,
        "refresh": {"requested": requested, "scope": "host", "result": if !requested { "notRequested" } else if refresh_error.is_some() { "failed" } else if updated { "updated" } else { "cached" }, "error": refresh_error},
    })
}

fn render(value: &Value) -> String {
    let account = &value["account"];
    let status = match value["state"].as_str() {
        Some("fresh") => "up to date",
        Some("stale") => "last known usage",
        _ => "unavailable",
    };
    let fetched = value["usageFetchedAt"].as_i64().map_or_else(
        || "not fetched yet".into(),
        |at| format!("{at} (Unix milliseconds)"),
    );
    let mut lines = vec![
        format!(
            "Codex account {} on {} ({})",
            account["id"].as_str().unwrap_or(""),
            value["deviceId"].as_str().unwrap_or(""),
            account["planLabel"].as_str().unwrap_or("plan unavailable")
        ),
        format!("Usage: {status}; last updated: {fetched}"),
    ];
    for window in value["windows"].as_array().into_iter().flatten() {
        let used = window["usedPercent"].as_f64().map_or_else(
            || "usage unavailable".into(),
            |used| format!("{used:.1}% used"),
        );
        let reset = window["resetsAt"].as_str().unwrap_or("time unavailable");
        lines.push(format!(
            "{}: {used}; resets {reset}",
            window["label"].as_str().unwrap_or("Usage")
        ));
    }
    for error in [&value["usageError"], &value["refresh"]["error"]] {
        if let Some(error) = error.as_str() {
            lines.push(error.into());
        }
    }
    if value["refresh"]["requested"] == true {
        lines.push(format!(
            "Refresh: {}",
            value["refresh"]["result"].as_str().unwrap_or("cached")
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_rpc::{RpcError, RpcReply, RpcService, memory_client};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    const HOST: &str = "00000000-0000-4000-8000-000000000001";

    fn account() -> AgentAccount {
        serde_json::from_value(json!({
            "id": "test-account", "harness": "codex", "active": true,
            "email": null, "planLabel": "Test plan", "usageFetchedAt": 1000,
            "usageWindows": [{"label": "5 hours", "usedFraction": 0.25, "resetsAt": null}],
        }))
        .unwrap()
    }

    fn options() -> Options {
        Options {
            device: HOST.into(),
            account: "active".into(),
            refresh: false,
            port: None,
        }
    }

    #[test]
    fn selection_is_exact_and_scoped_to_codex() {
        let a = account();
        let mut other = a.clone();
        other.harness = HarnessId::ClaudeCode;
        let mut snapshot = AgentAccountsSnapshot {
            accounts: vec![other, a.clone()],
            warnings: vec![],
        };
        assert_eq!(select(&snapshot, "active").unwrap().id, a.id);
        assert!(select(&snapshot, "test").is_err());
        snapshot.accounts.push(a);
        assert!(
            select(&snapshot, "active")
                .unwrap_err()
                .contains("More than one")
        );
        assert!(select(&AgentAccountsSnapshot::default(), "active").is_err());
    }

    #[test]
    fn freshness_preserves_errors_nulls_and_last_good_windows() {
        let mut a = account();
        let read = |a: &AgentAccount, now| report(HOST, a, now, false, false, None);
        assert_eq!(read(&a, 301_000)["state"], "fresh");
        assert_eq!(read(&a, 301_000)["stale"], false);
        assert_eq!(read(&a, 301_000)["cachedAt"], 1000);
        assert_eq!(read(&a, 301_001)["state"], "stale");
        assert_eq!(read(&a, 301_001)["stale"], true);
        assert_eq!(read(&a, 0)["state"], "stale");
        assert!(read(&a, 0)["ageMs"].is_null());
        a.usage_error = Some("Rate limited — retrying in 2m".into());
        let value = read(&a, 2000);
        assert_eq!(value["state"], "stale");
        assert_eq!(value["windows"][0]["usedPercent"], 25.0);
        assert_eq!(value["windows"][0]["usedFraction"], 0.25);
        assert!(value["windows"][0]["resetsAt"].is_null());
        assert_eq!(value["usageError"], a.usage_error.clone().unwrap());
        a.usage_windows.clear();
        assert_eq!(read(&a, 2000)["state"], "unavailable");
        a.usage_fetched_at = None;
        a.usage_error = None;
        let value = read(&a, 2000);
        assert_eq!(value["state"], "neverFetched");
        assert_eq!(value["stale"], true);
        assert!(value["cachedAt"].is_null());
        assert_eq!(value["available"], false);
        assert!(value["usageFetchedAt"].is_null());
        assert!(value["ageMs"].is_null());
        assert_eq!(value["windows"], json!([]));
        a.plan_label = None;
        assert!(read(&a, 2000)["account"]["planLabel"].is_null());
    }

    #[test]
    fn reset_after_fetch_marks_the_observation_stale() {
        let mut a = account();
        a.usage_windows[0].resets_at =
            Some(serde_json::from_value(json!("1970-01-01T00:00:02Z")).unwrap());
        let value = report(HOST, &a, 2000, false, false, None);
        assert_eq!(value["state"], "stale");
        assert_eq!(value["windows"][0]["resetsAt"], "1970-01-01T00:00:02Z");
    }

    struct Script(Mutex<VecDeque<(&'static str, Value, Result<Value, RpcError>)>>);

    #[async_trait::async_trait]
    impl RpcService for Script {
        async fn handle(&self, method: &str, params: Value) -> Result<RpcReply, RpcError> {
            let (expected_method, expected_params, response) =
                self.0.lock().unwrap().pop_front().expect("unexpected RPC");
            assert_eq!(method, expected_method);
            assert_eq!(params, expected_params);
            response.map(RpcReply::Value)
        }
    }

    fn step(a: AgentAccount, refresh: bool) -> (&'static str, Value, Result<Value, RpcError>) {
        (
            methods::LIST_AGENT_ACCOUNTS,
            json!({"targetDeviceId": HOST, "forceUsage": refresh}),
            Ok(json!({"accounts": [a], "warnings": []})),
        )
    }

    #[tokio::test]
    async fn default_uses_only_shared_cache_and_resolves_local_identity() {
        let script = Arc::new(Script(Mutex::new(VecDeque::from([
            (
                methods::LOCAL_DEVICE,
                json!({}),
                Ok(json!({"deviceId": HOST})),
            ),
            step(account(), false),
        ]))));
        let client = memory_client(script.clone());
        let mut options = options();
        options.device = "local".into();
        let value = query(&client, &options, || 2000).await.unwrap();
        assert_eq!(value["deviceId"], HOST);
        assert_eq!(value["refresh"]["result"], "notRequested");
        assert!(script.0.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn missing_account_does_not_refresh_other_accounts() {
        let script = Arc::new(Script(Mutex::new(VecDeque::from([step(account(), false)]))));
        let mut options = options();
        options.account = "missing-account".into();
        options.refresh = true;
        assert!(
            query(&memory_client(script.clone()), &options, || 2000)
                .await
                .is_err()
        );
        assert!(script.0.lock().unwrap().is_empty());
    }

    #[test]
    fn usage_copy_passes_the_plain_words_guard() {
        assert!(oa_copy::violations(USAGE, &[]).is_empty());
        let value = report(HOST, &account(), 2000, false, false, None);
        assert!(oa_copy::violations(&render(&value), &[]).is_empty());
    }

    #[tokio::test]
    async fn refresh_honors_engine_cached_and_backoff_responses() {
        for error in [None, Some("Rate limited — retrying in 2m")] {
            let mut after = account();
            after.usage_error = error.map(str::to_owned);
            let script = Arc::new(Script(Mutex::new(VecDeque::from([
                step(account(), false),
                step(after, true),
            ]))));
            let mut options = options();
            options.refresh = true;
            let value = query(&memory_client(script.clone()), &options, || 2000)
                .await
                .unwrap();
            assert_eq!(value["refresh"]["result"], "cached");
            assert_eq!(value["usageFetchedAt"], 1000);
            assert_eq!(value["usageError"], json!(error));
            assert_eq!(value["windows"][0]["usedPercent"], 25.0);
            assert!(script.0.lock().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn refresh_preserves_selection_and_reports_new_observation() {
        let mut after = account();
        after.active = false;
        after.usage_fetched_at = Some(2000);
        let script = Arc::new(Script(Mutex::new(VecDeque::from([
            step(account(), false),
            step(after, true),
        ]))));
        let mut options = options();
        options.refresh = true;
        let value = query(&memory_client(script), &options, || 2001)
            .await
            .unwrap();
        assert_eq!(value["account"]["id"], "test-account");
        assert_eq!(value["account"]["active"], false);
        assert_eq!(value["refresh"]["result"], "updated");
    }

    #[tokio::test]
    async fn failed_refresh_keeps_last_good_data_and_hides_transport_details() {
        let script = Arc::new(Script(Mutex::new(VecDeque::from([
            step(account(), false),
            (
                methods::LIST_AGENT_ACCOUNTS,
                json!({"targetDeviceId": HOST, "forceUsage": true}),
                Err(RpcError::Failed("private transport details".into())),
            ),
        ]))));
        let mut options = options();
        options.refresh = true;
        let value = query(&memory_client(script), &options, || 2000)
            .await
            .unwrap();
        assert_eq!(value["refresh"]["result"], "failed");
        assert_eq!(value["state"], "stale");
        assert_eq!(value["windows"][0]["usedPercent"], 25.0);
        assert!(!value.to_string().contains("private transport"));
    }

    #[test]
    fn requires_explicit_host_and_rejects_unsupported_options() {
        for words in [
            vec![],
            vec!["--device", "laptop"],
            vec!["--device", "local", "--harness", "claude-code"],
            vec!["--device", "local", "--ipc-port", "0"],
            vec!["--device", "local", "--force"],
        ] {
            assert!(
                Options::parse(&words.iter().map(|s| s.to_string()).collect::<Vec<_>>()).is_err()
            );
        }
    }
}
