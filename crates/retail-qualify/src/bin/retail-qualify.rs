//! `retail-qualify`: the retail cloud's acceptance run, funded-qualification
//! plan, and launch gate. See `docs/cloud/retail-qualification.md`.

use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!(
        "usage: retail-qualify accept [--out PATH]\n       retail-qualify plan [--plan PATH]\n       retail-qualify qualify --fake [--plan PATH] [--out PATH]\n       retail-qualify qualify --simulated [--plan PATH] [--out PATH]\n       retail-qualify qualify --funded --confirm PLAN_DIGEST [--bindings PATH] [--plan PATH] [--out PATH]\n       retail-qualify advertise [--contract-confirmed] [--receipt PATH] [--plan PATH]\n       retail-qualify health --journal PATH --ledger PATH"
    );
    ExitCode::from(2)
}

fn write(json: &str, out: Option<&String>) -> ExitCode {
    match out {
        Some(path) => match std::fs::write(path, format!("{json}\n")) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("cannot write {path}: {error}");
                ExitCode::FAILURE
            }
        },
        None => {
            println!("{json}");
            ExitCode::SUCCESS
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1));
    match args.first().map(String::as_str) {
        Some("accept") => {
            let receipt = retail_qualify::acceptance::run();
            let Ok(json) = serde_json::to_string_pretty(&receipt) else {
                return ExitCode::FAILURE;
            };
            let code = write(&json, out);
            if receipt.passed {
                code
            } else {
                ExitCode::FAILURE
            }
        }
        Some("plan") => match load_plan(&args) {
            Ok(plan) => match plan.check() {
                Ok(()) => {
                    println!("{}", plan.digest());
                    ExitCode::SUCCESS
                }
                Err(refusal) => {
                    eprintln!("plan refused: {refusal:?}");
                    ExitCode::FAILURE
                }
            },
            Err(code) => code,
        },
        Some("qualify") => {
            let plan = match load_plan(&args) {
                Ok(plan) => plan,
                Err(code) => return code,
            };
            if args.iter().any(|a| a == "--funded") {
                let confirm = args
                    .iter()
                    .position(|a| a == "--confirm")
                    .and_then(|i| args.get(i + 1))
                    .cloned()
                    .unwrap_or_default();
                let bindings = match flag(&args, "--bindings") {
                    Some(path) => match std::fs::read_to_string(path)
                        .map_err(|e| e.to_string())
                        .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
                    {
                        Ok(bindings) => Some(bindings),
                        Err(error) => {
                            eprintln!("cannot read the bindings {path}: {error}");
                            return ExitCode::FAILURE;
                        }
                    },
                    None => None,
                };
                let env = |name: &str| std::env::var(name).ok();
                return match retail_qualify::qualify::run_funded(
                    &plan,
                    &confirm,
                    bindings.as_ref(),
                    &env,
                ) {
                    Ok(receipt) => receipt_out(&receipt, out),
                    Err(refusal) => {
                        eprintln!(
                            "funded qualification refused: {refusal:?}. Follow docs/cloud/retail-qualification.md."
                        );
                        ExitCode::from(3)
                    }
                };
            }
            if args.iter().any(|a| a == "--simulated") {
                return receipt_out(&retail_qualify::qualify::run_simulated(&plan), out);
            }
            if !args.iter().any(|a| a == "--fake") {
                return usage();
            }
            receipt_out(&retail_qualify::qualify::run_fake(&plan), out)
        }
        Some("advertise") => {
            let plan = match load_plan(&args) {
                Ok(plan) => plan,
                Err(code) => return code,
            };
            let receipt = match flag(&args, "--receipt") {
                Some(path) => match std::fs::read_to_string(path)
                    .ok()
                    .and_then(|t| serde_json::from_str(&t).ok())
                {
                    Some(receipt) => Some(receipt),
                    None => {
                        eprintln!("cannot read the receipt {path}");
                        return ExitCode::FAILURE;
                    }
                },
                None => None,
            };
            let gate = retail_qualify::launch::Gate {
                contract_confirmed: args.iter().any(|a| a == "--contract-confirmed"),
                qualification: receipt,
                supported_plan: plan.digest(),
                capacity: retail_cloud::offer::Capacity {
                    running: 0,
                    plan_starts_left: None,
                },
            };
            let advertisement = retail_qualify::launch::advertise(&gate);
            match serde_json::to_string_pretty(&advertisement) {
                Ok(json) => write(&json, out),
                Err(_) => ExitCode::FAILURE,
            }
        }
        Some("health") => {
            let (Some(journal), Some(ledger)) = (flag(&args, "--journal"), flag(&args, "--ledger"))
            else {
                return usage();
            };
            let opened = retail_cloud::journal::Journal::open(journal)
                .and_then(|j| Ok((j, pay_ledger::Ledger::open(ledger)?)));
            let Ok((journal, ledger)) = opened else {
                eprintln!("cannot open the journal or the ledger");
                return ExitCode::FAILURE;
            };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
            match retail_qualify::launch::health(&journal, &ledger, now) {
                Ok(alerts) => {
                    let json = serde_json::to_string_pretty(&alerts).unwrap_or_default();
                    let code = write(&json, out);
                    if alerts.is_empty() {
                        code
                    } else {
                        ExitCode::from(4)
                    }
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => usage(),
    }
}

/// Write a qualification receipt; fail unless it qualified.
fn receipt_out(
    receipt: &retail_qualify::qualify::QualificationReceipt,
    out: Option<&String>,
) -> ExitCode {
    let Ok(json) = serde_json::to_string_pretty(receipt) else {
        return ExitCode::FAILURE;
    };
    let code = write(&json, out);
    if receipt.qualified {
        code
    } else {
        ExitCode::FAILURE
    }
}

fn load_plan(args: &[String]) -> Result<retail_qualify::qualify::Plan, ExitCode> {
    let Some(path) = args
        .iter()
        .position(|a| a == "--plan")
        .and_then(|i| args.get(i + 1))
    else {
        return Ok(retail_qualify::qualify::fixture());
    };
    let text = std::fs::read_to_string(path).map_err(|error| {
        eprintln!("cannot read {path}: {error}");
        ExitCode::FAILURE
    })?;
    serde_json::from_str(&text).map_err(|error| {
        eprintln!("cannot parse {path}: {error}");
        ExitCode::FAILURE
    })
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
}
