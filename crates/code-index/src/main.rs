use code_index::episodes::{self, Limits};
use std::path::PathBuf;

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |key: &str| -> Option<String> {
        args.iter()
            .position(|a| a == key)
            .and_then(|at| args.get(at + 1))
            .cloned()
    };
    if args.first().map(String::as_str) != Some("import") {
        return Err("Use code-index import --repo DIR --out DIR with --claude ROOT, --codex ROOT, or --coder ROOT.".into());
    }
    let repo = PathBuf::from(value("--repo").ok_or("Choose the repository with --repo DIR.")?)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let out =
        PathBuf::from(value("--out").ok_or("Choose a private output directory with --out DIR.")?);
    let history = coder_history::History::open(coder_history::Config {
        claude: value("--claude").map(PathBuf::from),
        codex: value("--codex").map(PathBuf::from),
        coder: value("--coder").map(PathBuf::from),
        ..Default::default()
    })
    .map_err(|e| e.to_string())?;
    let before_ms = value("--before")
        .map(|v| atif::parse_iso(&v).ok_or("Use an ISO 8601 date for --before."))
        .transpose()?;
    let corpus = episodes::import(
        &history,
        &repo,
        Limits {
            before_ms,
            ..Limits::default()
        },
    )?;
    episodes::save(&out, &corpus)?;
    println!("{}", episodes::summary(&corpus));
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
