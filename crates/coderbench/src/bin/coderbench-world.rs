//! Drive a world episode and grade its trace with independent workspace observations.
use coder_boundary::snapshot::Snapshot;
use coderbench::{Ending, Task, Workspace, observe};
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::Duration;

fn run() -> Result<bool, String> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 5 {
        return Err(
            "usage: coderbench-world TASK_JSON WORLD_JSON VOYAGER WORKSPACE RUNS_DIR".into(),
        );
    }
    let task = Task::load(&args[0])?;
    let runs = args[4].join(format!(
        "world-grade-{}-{}",
        std::process::id(),
        atif::document::now_ms()
    ));
    std::fs::create_dir_all(&runs).map_err(|e| e.to_string())?;
    let before = Snapshot::observe(&args[3]);
    let mut command = Command::new(&args[2]);
    command
        .args(["run", "--world"])
        .arg(&args[1])
        .arg("--runs")
        .arg(&runs)
        .current_dir(&args[3]);
    let started = std::time::Instant::now();
    let ran = coderbench::drive::output(command, Duration::from_secs(task.timeout_secs));
    let after = Snapshot::observe(&args[3]);
    let ran = ran?;
    std::fs::write(runs.join("stdout.log"), &ran.out).map_err(|e| e.to_string())?;
    std::fs::write(runs.join("stderr.log"), &ran.err).map_err(|e| e.to_string())?;
    let traces: Vec<_> = std::fs::read_dir(&runs)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path().join("trace.jsonl"))
        .filter(|p| p.is_file())
        .collect();
    if traces.len() != 1 {
        return Err("expected one episode trace".into());
    }
    let trace = &traces[0];
    let recording = atif::log::read(trace).map_err(|e| e.to_string())?;
    if !recording.session.model.starts_with("voyager/") {
        return Err("expected a Voyager episode".into());
    }
    let mut observed = observe(trace)?;
    observed.ending = if ran.code == Some(0) {
        Ending::Other(recording.session.state.clone())
    } else {
        Ending::Failed
    };
    observed.workspace = Some(Workspace::between(&before, &after)?);
    let judgment = task.judge(&observed);
    let result = serde_json::json!({"schema":"openagents.coderbench.world-run/v1","task":task.id,
        "passed":judgment.passed(),"faults":format!("{:?}",judgment.faults),"trace":trace,
        "world":recording.session.repository,"exit_success":(ran.code==Some(0)),"seconds":started.elapsed().as_secs_f64(),
        "workspace":format!("{:?}",observed.workspace)});
    std::fs::write(
        runs.join("grade.json"),
        serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("{result}");
    Ok(judgment.passed())
}
fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("coderbench-world: {e}");
            ExitCode::FAILURE
        }
    }
}
