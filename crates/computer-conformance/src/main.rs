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
    #[cfg(feature = "space")]
    let mut space = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            #[cfg(feature = "space")]
            "--space" => space = true,
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
                    "computer-conformance --boat https://HOST/api/v1 [--size NAME]\ncomputer-conformance --space [--size NAME] (build with --features space)\n\nBOAT_API_KEY holds the Boat key. SPACE reads SPACE_API_URL, SPACE_DATA_PLANE_URL, SPACE_API_TOKEN, and SPACE_TEMPLATE."
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
    #[cfg(feature = "space")]
    if space {
        if url.is_some() {
            eprintln!("Choose either --boat or --space.");
            return ExitCode::from(2);
        }
        return match computer_driver::space::SpaceDriver::from_env() {
            Ok(driver) => run(driver, size).await,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(2)
            }
        };
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
    run(driver, size).await
}

async fn run(driver: impl computer_driver::Driver, size: Option<String>) -> ExitCode {
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
