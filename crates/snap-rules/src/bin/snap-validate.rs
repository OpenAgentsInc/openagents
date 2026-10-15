//! Check the shipped jobs, or one draft job file.

fn main() {
    let paths: Vec<_> = std::env::args().skip(1).collect();
    let result = if paths.is_empty() {
        (1..=snap_rules::level::COUNT).try_for_each(|n| check(snap_rules::level::job(n)))
    } else {
        paths.iter().try_for_each(|p| {
            let text =
                std::fs::read_to_string(p).map_err(|e| format!("Could not read {p}: {e}"))?;
            check(snap_rules::level::parse(&text)?)
        })
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn check(level: snap_rules::Level) -> Result<(), String> {
    let report = snap_rules::validate::check(&level)
        .map_err(|e| format!("Job {} ({}): {e}", level.number, level.name))?;
    let unit = if report.pieces == 1 {
        "piece"
    } else {
        "pieces"
    };
    println!(
        "Job {}: {} — {} {unit}, three stars.",
        level.number, level.name, report.pieces
    );
    Ok(())
}
