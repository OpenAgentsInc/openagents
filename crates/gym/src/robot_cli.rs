//! Browse and verify robot results without starting a controller or upload.
use crate::robot::{self, Partition, Row, Suite};
use robot_lab::{Reference, Store as Lab, document::Build};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::Path,
};
pub fn command(args: &[String], output: &mut impl Write) -> Result<(), String> {
    let Some(verb) = args.first() else {
        return Err("Use gym robot record, report, compare, replay, or publish".into());
    };
    let mut options = BTreeMap::new();
    let mut json = false;
    let mut flags = args[1..].iter();
    while let Some(key) = flags.next() {
        if key == "--json" {
            json = true;
            continue;
        }
        let value = flags
            .next()
            .ok_or("Choose a value for this robot-result option")?;
        if ![
            "--lab-root",
            "--project",
            "--suite",
            "--results",
            "--episode",
            "--partition",
            "--controller",
            "--other-controller",
            "--player",
            "--output",
        ]
        .contains(&key.as_str())
            || options.insert(key.as_str(), value.as_str()).is_some()
        {
            return Err("Choose supported robot-result options once each".into());
        }
    }
    let required = |name| {
        options
            .get(name)
            .copied()
            .ok_or_else(|| format!("Choose {name} for these robot results"))
    };
    let lab = Lab::open(Path::new(required("--lab-root")?), required("--project")?)
        .map_err(|e| e.to_string())?;
    let parse_reference = |text: &str| -> Result<Reference, String> {
        let (document, version) = text
            .split_once('@')
            .ok_or("Choose a saved attempt as DOCUMENT@VERSION")?;
        let reference = Reference {
            document: document.into(),
            version: version.into(),
        };
        reference.validate().map_err(|e| e.to_string())?;
        Ok(reference)
    };
    if verb == "replay" {
        let reference = parse_reference(required("--episode")?)?;
        let version = lab.get(&reference).map_err(|e| e.to_string())?;
        if !matches!(version.content, robot_lab::Document::Episode(_)) {
            return Err("Choose a saved robot attempt to replay".into());
        }
        let base = player(required("--player")?)?;
        let link = format!(
            "{base}/play?document={}&version={}",
            reference.document, reference.version
        );
        if json {
            writeln!(
                output,
                "{}",
                serde_json::json!({"episode":reference,"player":link})
            )
            .map_err(|e| e.to_string())?;
        } else {
            writeln!(output, "Open this attempt: {link}").map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    let path = Path::new(required("--suite")?);
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| "This robot suite couldn't be opened")?
        .take((2 << 20) + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "This robot suite couldn't be read")?;
    if bytes.len() > 2 << 20 {
        return Err("Choose a robot suite smaller than 2 MB".into());
    }
    let suite: Suite =
        serde_json::from_slice(&bytes).map_err(|_| "This robot suite couldn't be read")?;
    suite.validate()?;
    let partition = match options.get("--partition").copied().unwrap_or("validation") {
        "training" => Partition::Training,
        "validation" => Partition::Validation,
        "held_out" => Partition::HeldOut,
        _ => return Err("Choose training, validation, or held_out".into()),
    };
    let results = Path::new(required("--results")?);
    if verb == "record" {
        private_path(results)?;
        if !results.exists() {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options
                .open(results)
                .map_err(|_| "The results file couldn't be created")?;
        }
        let reference = parse_reference(required("--episode")?)?;
        let row = robot::row(&lab, &suite, partition, &reference)?;
        let saved = crate::store::Store::at(results)
            .append(&row)
            .map_err(|_| "This result already exists, or the results file couldn't be saved")?;
        if json {
            writeln!(output, "{saved}").map_err(|e| e.to_string())?;
        } else {
            writeln!(
                output,
                "Attempt {} saved: {}",
                row.seed,
                if row.metrics.success {
                    "succeeded"
                } else {
                    "didn't finish the task"
                }
            )
            .map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    let rows = robot::verify_rows(&lab, &suite, results)?;
    if verb == "publish" {
        let destination = Path::new(required("--output")?);
        private_path(destination)?;
        let selected = rows
            .iter()
            .filter(|r| r.partition == partition)
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err("Record attempts before publishing their results".into());
        }
        // Publish metrics and immutable references, never camera bytes,
        // owner paths, credentials, or episode notes.
        let publication = serde_json::json!({
            "schema":"openagents.gym.robot_publication.v1",
            "suite":suite,
            "partition":partition,
            "rows":selected,
            "notice":"Recorded simulation results. The file includes no recordings or camera images."
        });
        let bytes = serde_json::to_vec_pretty(&publication).map_err(|e| e.to_string())?;
        if bytes.len() > 8 << 20 {
            return Err("Publish a smaller set of robot results".into());
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(destination)
            .map_err(|_| "Choose a new file for the published results")?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "The results file couldn't be saved")?;
        writeln!(output,"{}",serde_json::json!({"sha256":robot_lab::store::digest(&bytes),"attempts":selected.len(),"output":destination})).map_err(|e|e.to_string())?;
        return Ok(());
    }
    let build = |hash: &str| -> Result<Build, String> {
        rows.iter()
            .find(|row| row.controller.sha256 == hash)
            .map(|row| row.controller.clone())
            .ok_or("This controller has no recorded results in the file".into())
    };
    if verb == "compare" {
        let left = build(required("--controller")?)?;
        let right = build(required("--other-controller")?)?;
        let comparison = robot::compare(&suite, partition, &left, &right, &rows)?;
        if json {
            writeln!(
                output,
                "{}",
                serde_json::to_string(&comparison).map_err(|e| e.to_string())?
            )
            .map_err(|e| e.to_string())?;
        } else {
            for summary in [&comparison.left, &comparison.right] {
                write_summary(output, summary, options.get("--player").copied())?;
            }
            writeln!(output, "{}", comparison.conclusion).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    if verb != "report" {
        return Err("Use gym robot record, report, compare, replay, or publish".into());
    }
    let controllers = if let Some(hash) = options.get("--controller") {
        vec![build(hash)?]
    } else {
        let mut all = BTreeMap::new();
        for row in &rows {
            all.insert(row.controller.sha256.clone(), row.controller.clone());
        }
        all.into_values().collect()
    };
    let summaries = controllers
        .iter()
        .map(|controller| robot::summary(&suite, partition, controller, &rows))
        .collect::<Result<Vec<_>, _>>()?;
    if json {
        writeln!(output,"{}",serde_json::to_string(&serde_json::json!({"suite":suite.digest,"partition":partition,"controllers":summaries})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    } else {
        if summaries.is_empty() {
            writeln!(output, "No recorded attempts yet").map_err(|e| e.to_string())?;
        }
        for summary in &summaries {
            write_summary(output, summary, options.get("--player").copied())?;
        }
    }
    Ok(())
}
fn private_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("Choose an absolute path for robot results".into());
    }
    let parent = path
        .parent()
        .ok_or("Choose a folder for robot results")?
        .canonicalize()
        .map_err(|_| "Create the results folder before saving")?;
    if parent.ancestors().any(|p| p.join(".git").exists()) {
        return Err("Keep robot results outside the Git checkout".into());
    }
    if path.exists()
        && !std::fs::symlink_metadata(path)
            .map_err(|e| e.to_string())?
            .is_file()
    {
        return Err("Choose an ordinary file for robot results".into());
    }
    Ok(())
}
fn write_summary(
    output: &mut impl Write,
    summary: &robot::Summary,
    base: Option<&str>,
) -> Result<(), String> {
    writeln!(
        output,
        "{}: {} of {} recorded attempts succeeded",
        controller_name(&summary.controller),
        summary.successes,
        summary.recorded
    )
    .map_err(|e| e.to_string())?;
    if !summary.missing_seeds.is_empty() {
        writeln!(
            output,
            "{} attempts are missing",
            summary.missing_seeds.len()
        )
        .map_err(|e| e.to_string())?;
    }
    if let Some([low, high]) = summary.interval_95 {
        writeln!(
            output,
            "95% interval: {:.1}% to {:.1}%",
            low * 100.,
            high * 100.
        )
        .map_err(|e| e.to_string())?;
    }
    for row in summary.attempts.iter().filter(|r| !r.metrics.success) {
        let link = if let Some(base) = base {
            format!(
                " · {}/play?document={}&version={}",
                player(base)?,
                row.episode.document,
                row.episode.version
            )
        } else {
            format!(" · {}@{}", row.episode.document, row.episode.version)
        };
        writeln!(
            output,
            "Attempt {}: {}{}",
            row.seed,
            row.metrics
                .failure
                .as_deref()
                .unwrap_or("Didn't finish the task"),
            link
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn controller_name(build: &Build) -> &str {
    match build.name.as_str() {
        "robot-scripted-v1" => "Scripted controller",
        "robot-act-candle-v1" => "ACT",
        "robot-smolvla-candle-v1" => "SmolVLA",
        "robot-policy-service-v1" => "Policy service",
        _ => "Controller",
    }
}
fn player(text: &str) -> Result<String, String> {
    let url = url::Url::parse(text).map_err(|_| "Choose the local Robot Lab player's address")?;
    if url.scheme() != "http"
        || !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Choose the local Robot Lab player's address".into());
    }
    Ok(text.trim_end_matches('/').to_owned())
}

/// Attach links after row verification; cards carry saved episode references,
/// never a server address supplied by an episode or model response.
pub fn replay_links(rows: &[Row], base: &str) -> Result<serde_json::Value, String> {
    let base = player(base)?;
    Ok(serde_json::json!(rows.iter().map(|row|serde_json::json!({"episode":row.episode,"seed":row.seed,"success":row.metrics.success,"player":format!("{base}/play?document={}&version={}",row.episode.document,row.episode.version)})).collect::<Vec<_>>()))
}
