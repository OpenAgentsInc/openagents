use std::{env, fs, process};

use psionic_compat::builtin_mlx_parity_harness_report;

fn usage() {
    eprintln!(
        "Usage: cargo run -p psionic-compat --example mlx_parity_harness_report -- [--only <family>] [--report <path>]"
    );
}

fn main() {
    let mut selected_families = Vec::new();
    let mut report_path = None;
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--only" => {
                let Some(family_id) = args.next() else {
                    eprintln!("missing family id after --only");
                    usage();
                    process::exit(1);
                };
                selected_families.push(family_id);
            }
            "--report" => {
                let Some(path) = args.next() else {
                    eprintln!("missing path after --report");
                    usage();
                    process::exit(1);
                };
                report_path = Some(path);
            }
            "--help" | "-h" => {
                usage();
                return;
            }
            other => {
                eprintln!("unknown argument: {other}");
                usage();
                process::exit(1);
            }
        }
    }

    let report = builtin_mlx_parity_harness_report();
    let report = match report.filter_to_families(&selected_families) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{error}");
            process::exit(1);
        }
    };

    let json = match serde_json::to_string_pretty(&report) {
        Ok(json) => format!("{json}\n"),
        Err(error) => {
            eprintln!("failed to serialize MLX parity harness report: {error}");
            process::exit(1);
        }
    };

    if let Some(path) = report_path {
        if let Err(error) = fs::write(&path, json) {
            eprintln!("failed to write report to {path}: {error}");
            process::exit(1);
        }
    } else {
        print!("{json}");
    }
}
