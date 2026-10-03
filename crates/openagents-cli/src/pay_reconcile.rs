//! `openagents pay reconcile`: the ledger against the receiver wallet and
//! the payout Spark wallet (`pay_ledger::reconcile`), on demand or from the
//! pay host's ten-minute timer.
//!
//! The receiver wallet is read only through the running node's
//! `control.sock` (never opened here). The payout Spark wallet is read from
//! a scratch copy of its store under the report directory, so the payout
//! worker's own store is never written. With `--resolve`, an `unknown`
//! payout whose wallet record proves its outcome is settled in the ledger;
//! nothing is ever sent.

use std::path::{Path, PathBuf};

use openagents_spark::model::Node as _;
use openagents_wallet::config::Network;
use openagents_wallet::{LightningWallet, PaymentDirection, PaymentRecord, PaymentStatus};
use pay_ledger::Ledger;
use pay_ledger::reconcile::{
    Direction, Report, Severity, Snapshot, State, Status, WalletPayment, WalletView,
};
use serde_json::{Value, json};

use crate::{Args, Output};

/// How many Spark payments one read lists.
const SPARK_LIST: u32 = 10_000;

fn now() -> i64 {
    i64::try_from(crate::relay::unix_now()).unwrap_or(i64::MAX)
}

fn receiver_payment(record: &PaymentRecord) -> WalletPayment {
    WalletPayment {
        reference: record.payment_hash.clone(),
        direction: match record.direction {
            PaymentDirection::Inbound => Direction::Inbound,
            PaymentDirection::Outbound => Direction::Outbound,
        },
        status: match record.status {
            PaymentStatus::Pending => Status::Pending,
            PaymentStatus::Succeeded => Status::Succeeded,
            PaymentStatus::Failed => Status::Failed,
        },
        amount_msat: record.amount_msat.and_then(|a| i64::try_from(a).ok()),
        fee_msat: record
            .fee_msat
            .and_then(|f| i64::try_from(f).ok())
            .unwrap_or(0),
        at: i64::try_from(record.updated_at).unwrap_or(0),
    }
}

/// The receiver wallet through the resident: every payment when the
/// resident can list them, otherwise the ledger's references looked up one
/// by one. `None` when no resident answers.
fn read_receiver(references: &[String]) -> (Option<WalletView>, Option<Network>, Option<String>) {
    let home = openagents_wallet::config::home();
    let network = openagents_wallet::WalletConfig::load(&home)
        .ok()
        .map(|c| c.network);
    let Some(wallet) = openagents_wallet::resident::RemoteWallet::probe(&home) else {
        return (
            None,
            network,
            Some("no running receiver node answered on control.sock".into()),
        );
    };
    let balance_msat = match wallet.balance() {
        Ok(b) => i64::try_from(b.lightning_total_sats).ok().map(|s| s * 1000),
        Err(e) => return (None, network, Some(format!("receiver balance: {e}"))),
    };
    if let Ok(records) = wallet.payments() {
        let view = WalletView {
            payments: records.iter().map(receiver_payment).collect(),
            complete: true,
            balance_msat,
        };
        return (Some(view), network, None);
    }
    let mut payments = vec![];
    for reference in references {
        let Ok(hash) = openagents_wallet::parse_hash32(reference) else {
            continue;
        };
        match wallet.lookup(hash) {
            Ok(Some(record)) => payments.push(receiver_payment(&record)),
            Ok(None) => {}
            Err(e) => return (None, network, Some(format!("receiver lookup: {e}"))),
        }
    }
    let view = WalletView {
        payments,
        complete: false,
        balance_msat,
    };
    (Some(view), network, None)
}

fn spark_network(network: Network) -> Option<openagents_spark::Network> {
    match network {
        Network::Bitcoin => Some(openagents_spark::Network::Mainnet),
        Network::Regtest => Some(openagents_spark::Network::Regtest),
        Network::Testnet | Network::Signet => None,
    }
}

/// Copy the payout worker's Spark store into `scratch` so this process
/// never writes the worker's file.
fn copy_store(home: &Path, scratch: &Path) -> Result<(), String> {
    let wallets = home.join("wallets");
    let Ok(entries) = std::fs::read_dir(&wallets) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let store = entry.path().join("store.json");
        if !store.is_file() {
            continue;
        }
        let target = scratch.join("wallets").join(entry.file_name());
        std::fs::create_dir_all(&target).map_err(|e| format!("scratch store: {e}"))?;
        std::fs::copy(&store, target.join("store.json"))
            .map_err(|e| format!("scratch store: {e}"))?;
    }
    Ok(())
}

fn read_spark(home: &Path, scratch: &Path, network: Network) -> Result<WalletView, String> {
    let seed = openagents_spark::computer::load_seed(home)?
        .ok_or("the payout Spark wallet's seed is not here (is the payout worker running?)")?;
    let network = spark_network(network).ok_or("Spark has no network for this wallet")?;
    copy_store(home, scratch)?;
    let node = openagents_spark::computer::open_with(scratch, &seed, network)?;
    node.sync()?;
    let balance = node.balance()?;
    let rows = node.payments(SPARK_LIST)?;
    let payments = rows
        .iter()
        .map(|row| WalletPayment {
            reference: row.id.clone(),
            direction: if row.received {
                Direction::Inbound
            } else {
                Direction::Outbound
            },
            status: match row.status.as_str() {
                "completed" => Status::Succeeded,
                "failed" => Status::Failed,
                _ => Status::Pending,
            },
            amount_msat: i64::try_from(row.amount_sats).ok().map(|s| s * 1000),
            fee_msat: i64::try_from(row.fee_sats).unwrap_or(0) * 1000,
            at: i64::try_from(row.at).unwrap_or(0),
        })
        .collect();
    Ok(WalletView {
        payments,
        // The list is complete unless it hit the limit.
        complete: rows.len() < SPARK_LIST as usize,
        balance_msat: i64::try_from(balance).ok().map(|s| s * 1000),
    })
}

fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The day's report: the latest run of that UTC day, with how many runs
/// there were and how many found drift.
pub(crate) fn daily(previous: Option<&Value>, report: &Value) -> Value {
    let day = |v: Option<&Value>, key: &str| v.and_then(|v| v["day"][key].as_i64()).unwrap_or(0);
    let drift = report["state"].as_str() == Some("drift");
    let mut kinds: Vec<String> = previous
        .and_then(|v| v["day"]["drift_kinds"].as_array())
        .map(|a| {
            a.iter()
                .filter_map(|k| k.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    for f in report["findings"].as_array().into_iter().flatten() {
        if f["severity"].as_str() == Some("drift")
            && let Some(kind) = f["kind"].as_str()
            && !kinds.iter().any(|k| k == kind)
        {
            kinds.push(kind.to_owned());
        }
    }
    kinds.sort();
    let first_drift = previous
        .and_then(|v| v["day"]["first_drift_at"].as_i64())
        .or_else(|| drift.then(|| report["at"].as_i64().unwrap_or(0)));
    let mut out = report.clone();
    out["day"] = json!({
        "runs": day(previous, "runs") + 1,
        "drift_runs": day(previous, "drift_runs") + i64::from(drift),
        "first_drift_at": first_drift,
        "drift_kinds": kinds,
        "resolved": day(previous, "resolved")
            + i64::try_from(report["resolved"].as_array().map_or(0, Vec::len)).unwrap_or(0),
    });
    out
}

fn daily_text(day: &Value, report: &Report) -> String {
    format!(
        "{}Today: {} runs, {} with drift{}; {} unknown payouts resolved.\n",
        report.text(),
        day["day"]["runs"],
        day["day"]["drift_runs"],
        day["day"]["drift_kinds"]
            .as_array()
            .filter(|k| !k.is_empty())
            .map(|k| {
                format!(
                    " ({})",
                    k.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .unwrap_or_default(),
        day["day"]["resolved"],
    )
}

/// Write `latest.json`/`latest.txt` and the day's `daily/DATE.json`/`.txt`.
/// Returns the previous run's state.
fn write_reports(dir: &Path, report: &Report, value: &Value) -> Result<Option<String>, String> {
    let daily_dir = dir.join("daily");
    std::fs::create_dir_all(&daily_dir).map_err(|e| format!("{}: {e}", daily_dir.display()))?;
    let read = |p: &Path| -> Option<Value> {
        serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
    };
    let previous =
        read(&dir.join("latest.json")).and_then(|v| v["state"].as_str().map(str::to_owned));
    let date = pay_ledger::reconcile::utc_date(report.at);
    let day_path = daily_dir.join(format!("{date}.json"));
    let day = daily(read(&day_path).as_ref(), value);
    let pretty = |v: &Value| serde_json::to_string_pretty(v).unwrap_or_default() + "\n";
    write_atomic(&day_path, &pretty(&day))?;
    write_atomic(
        &daily_dir.join(format!("{date}.txt")),
        &daily_text(&day, report),
    )?;
    write_atomic(&dir.join("latest.txt"), &report.text())?;
    // latest.json last: pay-host reads it for /stats.
    write_atomic(&dir.join("latest.json"), &pretty(value))?;
    Ok(previous)
}

/// A line for the journal at error priority (`<3>`, honoured by journald)
/// when run under systemd.
fn alert(line: &Value, error: bool) {
    let prefix = if std::env::var_os("JOURNAL_STREAM").is_some() {
        if error { "<3>" } else { "<5>" }
    } else {
        ""
    };
    eprintln!("{prefix}{line}");
}

/// `pay reconcile`.
pub(crate) fn reconcile(output: &Output, words: &[String], usage: &str) -> u8 {
    let args = match Args::parse(words, &["resolve"]) {
        Ok(args) => args,
        Err(message) => return output.usage("pay", &message, usage),
    };
    let Some(path) = args
        .option("ledger")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("OPENAGENTS_PAY_LEDGER").map(PathBuf::from))
    else {
        return output.usage("pay", "reconcile needs --ledger FILE", usage);
    };
    let spark_home = args
        .option("spark-home")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("OPENAGENTS_PAY_SPARK_HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/var/lib/openagents-pay/spark"));
    let report_dir = args.option("report-dir").map(PathBuf::from);
    let mut ledger = match Ledger::open(&path) {
        Ok(ledger) => ledger,
        Err(e) => return output.fail("pay", &format!("{}: {e}", path.display())),
    };
    let (receiver_refs, _) = match pay_ledger::reconcile::references(&ledger) {
        Ok(refs) => refs,
        Err(e) => return output.fail("pay", &e.to_string()),
    };
    let mut problems: Vec<String> = vec![];
    let (receiver, network, why) = read_receiver(&receiver_refs);
    problems.extend(why);
    let scratch_root = report_dir.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("oa-reconcile-{}", std::process::id()))
    });
    let scratch = scratch_root.join("spark-scratch");
    let spark = match network {
        None => {
            problems.push("the receiver wallet's network is unknown, so Spark was not read".into());
            None
        }
        Some(network) => match read_spark(&spark_home, &scratch, network) {
            Ok(view) => Some(view),
            Err(why) => {
                problems.push(format!("payout Spark wallet: {why}"));
                None
            }
        },
    };
    if report_dir.is_none() {
        let _ = std::fs::remove_dir_all(&scratch_root);
    }
    let snapshot = Snapshot { receiver, spark };
    let at = now();
    let resolved = if args.switch("resolve") {
        match pay_ledger::reconcile::resolve_unknown(&mut ledger, &snapshot, at) {
            Ok(resolved) => resolved,
            Err(e) => return output.fail("pay", &e.to_string()),
        }
    } else {
        vec![]
    };
    let mut report = match pay_ledger::reconcile::reconcile(&ledger, &snapshot, at) {
        Ok(report) => report,
        Err(e) => return output.fail("pay", &e.to_string()),
    };
    report.resolved = resolved;
    let mut value = serde_json::to_value(&report).unwrap_or_default();
    value["problems"] = json!(problems);

    for r in &report.resolved {
        alert(
            &json!({"event": "reconciliation_resolved", "payout": r.payout, "rail": r.rail,
                    "state": r.state}),
            false,
        );
    }
    let mut previous = None;
    if let Some(dir) = &report_dir {
        match write_reports(dir, &report, &value) {
            Ok(p) => previous = p,
            Err(why) => return output.fail("pay", &why),
        }
    }
    let drift: Vec<&pay_ledger::reconcile::Finding> = report
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Drift)
        .collect();
    if report.state == State::Drift {
        alert(
            &json!({"event": "reconciliation_drift", "drift": drift.len(),
                    "kinds": drift.iter().map(|f| f.kind).collect::<Vec<_>>(),
                    "references": drift.iter().filter_map(|f| f.reference.as_deref()).collect::<Vec<_>>()}),
            true,
        );
    } else if previous.as_deref() == Some("drift") {
        alert(
            &json!({"event": "reconciliation_cleared", "state": report.state.as_str()}),
            false,
        );
    }
    for problem in &problems {
        alert(
            &json!({"event": "reconciliation_unchecked", "why": problem}),
            false,
        );
    }
    output.line(&value, |_| {
        let mut text = report.text();
        for problem in &problems {
            text.push_str(&format!("unchecked: {problem}\n"));
        }
        text.trim_end().to_owned()
    });
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_daily_report_counts_runs_and_drift_kinds() {
        let ok = json!({"at": 1, "state": "ok", "findings": [], "resolved": []});
        let drift = json!({"at": 2, "state": "drift", "resolved": [{"payout": "p"}],
            "findings": [{"kind": "payout_missing", "severity": "drift"},
                         {"kind": "extra_inbound", "severity": "notice"}]});
        let first = daily(None, &ok);
        assert_eq!(first["day"]["runs"], 1);
        assert_eq!(first["day"]["drift_runs"], 0);
        assert!(first["day"]["first_drift_at"].is_null());
        let second = daily(Some(&first), &drift);
        assert_eq!(second["day"]["runs"], 2);
        assert_eq!(second["day"]["drift_runs"], 1);
        assert_eq!(second["day"]["first_drift_at"], 2);
        assert_eq!(second["day"]["drift_kinds"], json!(["payout_missing"]));
        assert_eq!(second["day"]["resolved"], 1);
        let third = daily(Some(&second), &ok);
        assert_eq!(third["state"], "ok");
        assert_eq!(third["day"]["drift_runs"], 1);
        assert_eq!(third["day"]["drift_kinds"], json!(["payout_missing"]));
    }
}
