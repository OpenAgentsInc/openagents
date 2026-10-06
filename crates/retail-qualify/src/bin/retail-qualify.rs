//! `retail-qualify`: the retail cloud's acceptance run, funded-qualification
//! plan, and launch gate. See `docs/cloud/retail-qualification.md`.

use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!(
        "usage: retail-qualify accept [--out PATH]\n       retail-qualify plan [--plan PATH]\n       retail-qualify qualify --fake [--plan PATH] [--out PATH]\n       retail-qualify qualify --funded --confirm PLAN_DIGEST [--plan PATH]"
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
                return match retail_qualify::qualify::run_funded(&plan, &confirm) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(refusal) => {
                        eprintln!(
                            "funded qualification refused: {refusal:?}. Follow docs/cloud/retail-qualification.md."
                        );
                        ExitCode::from(3)
                    }
                };
            }
            if !args.iter().any(|a| a == "--fake") {
                return usage();
            }
            let receipt = retail_qualify::qualify::run_fake(&plan);
            let Ok(json) = serde_json::to_string_pretty(&receipt) else {
                return ExitCode::FAILURE;
            };
            let code = write(&json, out);
            if receipt.qualified {
                code
            } else {
                ExitCode::FAILURE
            }
        }
        _ => usage(),
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
