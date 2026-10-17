//! Database-authorized operations for computer allocation limits and usage.
use oa_computers::{Db, Store, usage::Limits};

const USAGE: &str = "Use read ACCOUNT [AFTER], account-limits ACCOUNT FILE, service-limits FILE, economics FILE, or resolve ACCOUNT COMPUTER BOOT MAXIMUM_CPU_MS USED_CPU_MS REASON.";

fn argument(args: &mut impl Iterator<Item = String>) -> Result<String, String> {
    args.next()
        .ok_or_else(|| "A required argument is missing.".into())
}

async fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or_else(|| USAGE.to_owned())?;
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return Ok(());
    }
    if command == "economics" {
        use std::io::Read;
        let path = argument(&mut args)?;
        if args.next().is_some() {
            return Err("Too many arguments.".into());
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| "The cost report input could not be opened.".to_owned())?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "The cost report input could not be read.".to_owned())?;
        if bytes.len() > 1024 * 1024 {
            return Err("The cost report input exceeds 1 MiB.".into());
        }
        let window: oa_computers::economics::Window = serde_json::from_slice(&bytes)
            .map_err(|_| "The cost report input is invalid.".to_owned())?;
        let report = window.report().map_err(str::to_owned)?;
        println!(
            "{}",
            serde_json::to_string(&report)
                .map_err(|_| "The cost report could not be written.".to_owned())?
        );
        return Ok(());
    }
    if !matches!(
        command.as_str(),
        "read" | "account-limits" | "service-limits" | "resolve"
    ) {
        return Err(USAGE.into());
    }
    let subject = argument(&mut args)?;
    let dsn = std::env::var("OA_COMPUTERS_DATABASE_URL")
        .map_err(|_| "Set OA_COMPUTERS_DATABASE_URL.".to_owned())?;
    let store = Store::new(Db::connect(&dsn).await.map_err(|e| e.to_string())?);
    match command.as_str() {
        "read" => {
            let after = args
                .next()
                .map(|v| v.parse::<i64>())
                .transpose()
                .map_err(|_| "The usage cursor is invalid.".to_owned())?
                .unwrap_or(0);
            if args.next().is_some() {
                return Err("Too many arguments.".into());
            }
            let usage = store
                .usage(&subject, after, 200)
                .await
                .map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::to_string(&usage).map_err(|e| e.to_string())?
            );
        }
        "account-limits" | "service-limits" => {
            let (owner, path) = if command == "service-limits" {
                (None, subject.as_str())
            } else {
                (Some(subject.as_str()), "")
            };
            let path = if owner.is_some() {
                argument(&mut args)?
            } else {
                path.to_owned()
            };
            if args.next().is_some() {
                return Err("Too many arguments.".into());
            }
            let limits: Limits =
                serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            store
                .set_usage_limits(owner, &limits)
                .await
                .map_err(|e| e.to_string())?;
            println!("{{\"updated\":true}}");
        }
        "resolve" => {
            let computer = argument(&mut args)?;
            let boot = argument(&mut args)?
                .parse()
                .map_err(|_| "The boot number is invalid.".to_owned())?;
            let maximum = argument(&mut args)?
                .parse()
                .map_err(|_| "The maximum is invalid.".to_owned())?;
            let used = argument(&mut args)?
                .parse()
                .map_err(|_| "The usage is invalid.".to_owned())?;
            let reason = argument(&mut args)?;
            if args.next().is_some() {
                return Err("Too many arguments.".into());
            }
            store
                .resolve_usage_allowance(&subject, &computer, boot, maximum, used, &reason)
                .await
                .map_err(|e| e.to_string())?;
            println!("{{\"resolved\":true}}");
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
