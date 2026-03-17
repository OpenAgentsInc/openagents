use std::{
    env, fs,
    io::{self, Write},
    process,
};

use psionic_compat::builtin_mlx_parity_harness_report;

fn write_stderr_line(message: &str) {
    let _ = writeln!(io::stderr(), "{message}");
}

fn usage() {
    write_stderr_line(
        "Usage: cargo run -p psionic-compat --example mlx_parity_harness_report -- [--only <family>] [--report <path>]",
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
                    write_stderr_line("missing family id after --only");
                    usage();
                    process::exit(1);
                };
                selected_families.push(family_id);
            }
            "--report" => {
                let Some(path) = args.next() else {
                    write_stderr_line("missing path after --report");
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
                write_stderr_line(&format!("unknown argument: {other}"));
                usage();
                process::exit(1);
            }
        }
    }

    let report = builtin_mlx_parity_harness_report();
    let report = match report.filter_to_families(&selected_families) {
        Ok(report) => report,
        Err(error) => {
            write_stderr_line(&error.to_string());
            process::exit(1);
        }
    };

    let json = match serde_json::to_string_pretty(&report) {
        Ok(json) => format!("{json}\n"),
        Err(error) => {
            write_stderr_line(&format!(
                "failed to serialize MLX parity harness report: {error}"
            ));
            process::exit(1);
        }
    };

    if let Some(path) = report_path {
        if let Err(error) = fs::write(&path, json) {
            write_stderr_line(&format!("failed to write report to {path}: {error}"));
            process::exit(1);
        }
    } else {
        let mut stdout = io::stdout().lock();
        if let Err(error) = stdout.write_all(json.as_bytes()) {
            write_stderr_line(&format!("failed to write report to stdout: {error}"));
            process::exit(1);
        }
    }
}
