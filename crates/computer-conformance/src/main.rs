//! `computer-conformance --boat URL [--size NAME]`: run the conformance
//! cases against a Boat-shaped driver service (staging `oa-boat`, a
//! customer's driver). The API key is read from `BOAT_API_KEY`; it is
//! never printed. Every computer and snapshot the run makes is deleted.

use boat::{ApiKey, Client};
use computer_driver::boat::BoatDriver;
use std::process::ExitCode;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut url = None;
    let mut size = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--boat" => {
                url = args.get(i + 1).cloned();
                i += 1;
            }
            "--size" => {
                size = args.get(i + 1).cloned();
                i += 1;
            }
            "-h" | "--help" => {
                println!(
                    "computer-conformance --boat https://HOST/api/v1 [--size NAME]\n\nBOAT_API_KEY holds the service's key."
                );
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("unknown argument {other}");
                return ExitCode::from(2);
            }
        }
        i += 1;
    }
    let Some(url) = url else {
        eprintln!("--boat URL is required");
        return ExitCode::from(2);
    };
    let Some(key) = std::env::var("BOAT_API_KEY")
        .ok()
        .and_then(|k| ApiKey::new(k).ok())
    else {
        eprintln!("BOAT_API_KEY is unset or invalid");
        return ExitCode::from(2);
    };
    let client = match Client::builder(key).base_url(url).build() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("client: {e}");
            return ExitCode::from(2);
        }
    };
    let driver = match BoatDriver::connect(client).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut options = computer_conformance::Options::fast(&format!("conf-{stamp}"));
    options.poll = std::time::Duration::from_secs(5);
    options.deadline = std::time::Duration::from_secs(600);
    options.size = size;
    let report = computer_conformance::run(&driver, &options).await;
    print!("{}", report.summary());
    if report.passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
