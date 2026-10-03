use briefing_lab::{Components, Index, Issue, Options, Result};
use std::{collections::BTreeMap, env, fs, path::PathBuf, process, time::Instant};

fn main() {
    if let Err(error) = run() {
        eprintln!("briefing-lab: {error}");
        process::exit(1);
    }
}
fn run() -> Result<()> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_default();
    if command == "--help" || command.is_empty() {
        println!(
            "briefing-lab index --repo PATH --rev COMMIT_OR_REF --output FILE [--syntax]\nbriefing-lab preview --repo PATH --rev COMMIT_OR_REF --index FILE --issue-file FILE --output-dir DIR [--no-lexical] [--no-symbols] [--no-history] [--syntax]\n\nAll artifacts must be outside the inspected repository. The preview reads committed source only."
        );
        return Ok(());
    }
    if !matches!(command.as_str(), "index" | "preview") {
        return Err("Expected index or preview.".into());
    }
    let mut values = BTreeMap::new();
    let mut components = Components::default();
    let mut options = Options::default();
    let mut args = env::args().skip(2);
    while let Some(key) = args.next() {
        match key.as_str() {
            "--syntax" => options.syntax = true,
            "--no-lexical" if command == "preview" => components.lexical = false,
            "--no-symbols" if command == "preview" => components.symbols = false,
            "--no-history" if command == "preview" => components.history = false,
            "--repo" | "--rev" | "--output" | "--index" | "--issue-file" | "--output-dir" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("Missing value for {key}."))?;
                if values.insert(key.clone(), value).is_some() {
                    return Err(format!("Duplicate option: {key}").into());
                }
            }
            _ => return Err(format!("Unknown option: {key}").into()),
        }
    }
    let required = |key: &str| -> Result<String> {
        values
            .get(key)
            .cloned()
            .ok_or_else(|| format!("Missing {key}.").into())
    };
    let repo = PathBuf::from(required("--repo")?);
    let revision = required("--rev")?;
    if command == "index" {
        let output = PathBuf::from(required("--output")?);
        briefing_lab::check_output(&repo, &output)?;
        let index = briefing_lab::build_index_with_options(&repo, &revision, options)?;
        if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output, serde_json::to_vec(&index)?)?;
        println!(
            "Indexed {} files at {} in {:.3} ms: {}",
            index.files.len(),
            index.commit,
            index.index_ms,
            output.display()
        );
        return Ok(());
    }
    let total = Instant::now();
    let verify_start = Instant::now();
    let commit = briefing_lab::resolve(&repo, &revision)?;
    let verify_ms = verify_start.elapsed().as_secs_f64() * 1000.0;
    let load_start = Instant::now();
    let index: Index = serde_json::from_slice(&fs::read(required("--index")?)?)?;
    let issue_bytes = fs::read(required("--issue-file")?)?;
    let issue: Issue = serde_json::from_slice(&issue_bytes)?;
    let load_ms = load_start.elapsed().as_secs_f64() * 1000.0;
    let mut brief =
        briefing_lab::assemble_with_options(&repo, &index, &commit, issue, components, options)?;
    let directory = PathBuf::from(required("--output-dir")?);
    briefing_lab::check_output(&repo, &directory)?;
    briefing_lab::check_output(&repo, &directory.join("briefing.json"))?;
    briefing_lab::check_output(&repo, &directory.join("briefing.md"))?;
    brief
        .timings_ms
        .insert("revision_validation".into(), verify_ms);
    brief
        .timings_ms
        .insert("index_and_issue_load".into(), load_ms);
    brief
        .timings_ms
        .insert("original_index_build_separate".into(), index.index_ms);
    brief.timings_ms.insert(
        "warm_preview_before_output".into(),
        total.elapsed().as_secs_f64() * 1000.0,
    );
    brief.notes.push(format!(
        "Input issue JSON SHA-256: {}",
        briefing_lab::sha256(&issue_bytes)
    ));
    let serialize_start = Instant::now();
    let _ = serde_json::to_vec(&brief)?;
    let _ = briefing_lab::markdown(&brief);
    brief.timings_ms.insert(
        "output_serialization_sample".into(),
        serialize_start.elapsed().as_secs_f64() * 1000.0,
    );
    fs::create_dir_all(&directory)?;
    fs::write(
        directory.join("briefing.json"),
        serde_json::to_vec_pretty(&brief)?,
    )?;
    fs::write(
        directory.join("briefing.md"),
        briefing_lab::markdown(&brief),
    )?;
    println!(
        "Preview: {}\nStructured evidence: {}\nComplete local preview including output: {:.3} ms",
        directory.join("briefing.md").display(),
        directory.join("briefing.json").display(),
        total.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}
