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
    if args.first().map(String::as_str) == Some("pilot") {
        let repo = PathBuf::from(value("--repo").ok_or("Choose a repository with --repo DIR.")?);
        let out =
            PathBuf::from(value("--out").ok_or("Choose an output directory with --out DIR.")?);
        let context = code_index::recipes::Context::open(
            &repo,
            &value("--rev").unwrap_or_else(|| "HEAD".into()),
            &value("--workspace").unwrap_or_else(|| "local".into()),
        )?;
        let artifact = code_index::pilot::compile(&context)?;
        code_index::private::write(
            &out,
            "task-index.json",
            &serde_json::to_vec(&artifact).map_err(|e| e.to_string())?,
        )?;
        println!(
            "{}",
            serde_json::json!({"recipes":artifact.recipes.len(),"commit":artifact.snapshot.commit})
        );
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("demand") {
        let input =
            PathBuf::from(value("--input").ok_or("Choose investigations with --input FILE.")?);
        let out = PathBuf::from(
            value("--out").ok_or("Choose a private output directory with --out DIR.")?,
        );
        let corpus: episodes::Corpus = code_index::read_json(&input)?;
        let grouping = value("--grouping")
            .map(|p| {
                code_index::read_json::<code_index::demand::Grouping>(std::path::Path::new(&p))
            })
            .transpose()?;
        let assumptions = value("--assumptions")
            .map(|p| code_index::read_json(std::path::Path::new(&p)))
            .transpose()?
            .unwrap_or_default();
        let demand = code_index::demand::rank(&corpus, grouping.as_ref(), assumptions)?;
        code_index::private::write(
            &out,
            "demand.json",
            &serde_json::to_vec(&demand).map_err(|e| e.to_string())?,
        )?;
        code_index::private::write(
            &out,
            "grouping-request.json",
            &serde_json::to_vec(&code_index::demand::grouping_request(&corpus)?)
                .map_err(|e| e.to_string())?,
        )?;
        println!(
            "{}",
            serde_json::json!({"candidates":demand.candidates.len(),"episodes":corpus.episodes.len(),"cost_assumptions_complete":demand.candidates.iter().all(|c|c.expected_net_saving.is_some())})
        );
        return Ok(());
    }
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
