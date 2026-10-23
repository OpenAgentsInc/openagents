//! Collect paired answers through Pylon's native direct stage pipeline.
//!
//! Usage: qualification_answers LAUNCH_JSON SUITE_JSON ORACLE_ANSWERS OUT_DIR
//! [PROBES_JSON]. The paired report supplies the original oracle answers and
//! verified bundle metadata; every runtime token comes from the live group.

use std::{collections::BTreeSet, fs, path::Path, sync::Arc, time::Duration};

use pylon::{
    engine::Sampling,
    share::GroupLane,
    stage_group::{PromptMode, StageGroup, Transport},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn read(path: &str) -> Result<(Value, String), String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    Ok((value, format!("{:x}", Sha256::digest(&bytes))))
}

fn save(path: &Path, value: &Value) -> Result<(), String> {
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

async fn answer(group: &StageGroup, task: &Value, cap: u64) -> Result<Value, String> {
    let mode: PromptMode =
        serde_json::from_value(task["thinking_mode"].clone()).map_err(|e| e.to_string())?;
    let sampling = Sampling::new(0.0, 1.0, 0)?;
    let started = std::time::Instant::now();
    let mut run = group
        .generate_prompt_in(task["messages"].clone(), mode, cap, sampling)
        .await?;
    let request = run.request.clone();
    let generation = run.generation;
    let (mut ids, mut text, mut reasoning, mut content) =
        (Vec::new(), String::new(), String::new(), String::new());
    let (mut first, mut last) = (None, None);
    let (mut tokens_open, mut text_open, mut answer_open) = (true, true, true);
    while tokens_open || text_open || answer_open {
        tokio::select! {
            token = run.tokens.recv(), if tokens_open => match token {
                Some(token) => {
                    let now = std::time::Instant::now();
                    first.get_or_insert(now);
                    last = Some(now);
                    ids.push(token);
                },
                None => tokens_open = false,
            },
            piece = run.text.recv(), if text_open => match piece {
                Some(piece) => text.push_str(&piece),
                None => text_open = false,
            },
            piece = run.answer.recv(), if answer_open => match piece {
                Some(piece) => {
                    reasoning.push_str(&piece.reasoning);
                    content.push_str(&piece.content);
                }
                None => answer_open = false,
            },
        }
    }
    let done = run
        .done
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    if done.tokens != ids.len() as u64 || done.tokens > cap {
        return Err("the returned tokens differ from the group's completion count".into());
    }
    Ok(json!({
        "case": task["id"], "request": request, "generation": generation, "mode": mode,
        "prompt_tokens": null,
        "tokens": ids.len(), "ids": ids, "text": text,
        "reasoning": reasoning, "content": content,
        "ended": done.finish.as_deref() == Some("stop"), "finish": done.finish,
        "seconds": started.elapsed().as_secs_f64(), "profile": done.profile,
        "first_token_ms": first.map(|at| at.duration_since(started).as_secs_f64()*1000.0),
        "decode_token_ms": first.zip(last).filter(|_| ids.len()>1)
            .map(|(a,b)| b.duration_since(a).as_secs_f64()*1000.0/(ids.len()-1) as f64),
    }))
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if !(5..=6).contains(&args.len()) {
        return Err("pass LAUNCH_JSON SUITE_JSON ORACLE_ANSWERS OUT_DIR [PROBES_JSON]".into());
    }
    let (launch, launch_sha) = read(&args[1])?;
    let lane: GroupLane =
        serde_json::from_value(launch.pointer("/settings/group").unwrap_or(&launch).clone())
            .map_err(|e| e.to_string())?;
    lane.validate()?;
    if lane.transport != Transport::Direct || lane.build.is_none() || lane.max_output_tokens != 4096
    {
        return Err("qualification needs a pinned direct group with a 4096-token cap".into());
    }
    let (suite, suite_sha) = read(&args[2])?;
    let (baseline, baseline_sha) = read(&args[3])?;
    if baseline["stage_bundle_sha256"].as_str() != lane.build.as_deref()
        || baseline["manifest"] != lane.manifest
        || baseline["cap"] != 4096
    {
        return Err("the paired oracle report names another group or cap".into());
    }
    let tasks = suite["cases"].as_array().ok_or("suite has no cases")?;
    let original = baseline["answers"]
        .as_array()
        .ok_or("paired report has no answers")?;
    let task_ids: BTreeSet<_> = tasks.iter().filter_map(|t| t["id"].as_str()).collect();
    let original_ids: BTreeSet<_> = original.iter().filter_map(|t| t["case"].as_str()).collect();
    if tasks.is_empty()
        || task_ids.len() != tasks.len()
        || original_ids.len() != original.len()
        || task_ids != original_ids
    {
        return Err("the paired report does not cover exactly the suite".into());
    }
    let probes = if let Some(path) = args.get(5) {
        read(path)?.0
    } else {
        json!([])
    };
    let probes = probes.as_array().ok_or("runtime probes must be an array")?;
    let out = Path::new(&args[4]);
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::create_dir(out).map_err(|e| e.to_string())?;
    let group =
        StageGroup::start(lane.stage_config(), Arc::new(|line| eprintln!("{line}"))).await?;
    let result = async {
        tokio::time::timeout(Duration::from_secs(1000), async {
            let mut changes = group.watch();
            loop {
                let status = changes.borrow_and_update().clone();
                if status.ready {
                    break Ok(());
                }
                if let Some(failure) = status.failed {
                    break Err(failure);
                }
                changes.changed().await.map_err(|e| e.to_string())?;
            }
        })
        .await
        .map_err(|_| "group startup timed out".to_string())??;
        let status = group.status();
        save(
            &out.join("ready.json"),
            &serde_json::to_value(&status).map_err(|e| e.to_string())?,
        )?;
        let devices: Vec<_> = status
            .stages
            .iter()
            .map(|s| s.announced.as_ref().ok_or("missing stage identity"))
            .collect::<Result<_, _>>()?;
        let configs: Vec<_> = devices
            .iter()
            .map(|d| d.execution_config_sha256.clone())
            .collect();
        if serde_json::to_value(&configs).map_err(|e| e.to_string())?
            != baseline["execution_config_sha256"]
            || devices.iter().any(|d| {
                d.bundle != lane.build
                    || d.profile.as_deref() != baseline["profile"].as_str()
                    || d.manifest.as_deref() != Some(lane.manifest.as_str())
            })
        {
            return Err(
                "the live group differs from the paired report's verified configuration".into(),
            );
        }
        let mut report = json!({
            "schema":"psionic.deepseek_v41.g2_v2_answers.v1",
            "collector":"pylon::stage_group::StageGroup (native direct)",
            "transport":"direct", "suite_sha256":suite_sha, "launch_sha256":launch_sha,
            "paired_report_sha256":baseline_sha, "cap":4096,
            "runtime":baseline["runtime"], "profile":baseline["profile"],
            "stage_bundle_sha256":lane.build, "expected_bundle_sha256":lane.build,
            "stage_executable_sha256":baseline["stage_executable_sha256"],
            "execution_config_sha256":configs, "manifest":lane.manifest, "launch":"pinned",
            "oracle_manifest_sha256":baseline["oracle_manifest_sha256"],
            "timing_limits":{"step_ms":lane.step_timeout_ms,"prefill_ms":lane.prefill_timeout_ms,"request_seconds":lane.deadline_secs},
            "input_count_note":"This API does not return input counts; oracle_prompt_tokens is the reference count, not a reported native count.",
            "answers":[], "probes":[],
        });
        for task in tasks {
            let mut row = tokio::time::timeout(
                Duration::from_secs(lane.deadline_secs),
                answer(&group, task, 4096),
            )
            .await
            .map_err(|_| "answer deadline expired".to_string())??;
            let oracle = original.iter().find(|o| o["case"] == task["id"]).unwrap();
            row["oracle"] = oracle["oracle"].clone();
            row["oracle_prompt_tokens"] = oracle["prompt_tokens"].clone();
            row["same_ids_as_oracle"] = json!(row["ids"] == oracle["oracle"]["ids"]);
            row["same_ids_as_front"] = json!(row["ids"] == oracle["ids"]);
            report["answers"].as_array_mut().unwrap().push(row);
            save(&out.join("answers.partial.json"), &report)?;
        }
        for task in probes {
            let cap = task["cap"]
                .as_u64()
                .filter(|n| *n > 0 && *n <= 4096)
                .ok_or("invalid probe cap")?;
            let row = tokio::time::timeout(
                Duration::from_secs(lane.deadline_secs),
                answer(&group, task, cap),
            )
            .await
            .map_err(|_| "probe deadline expired".to_string())??;
            report["probes"].as_array_mut().unwrap().push(row);
            save(&out.join("answers.partial.json"), &report)?;
        }
        Ok::<_, String>(report)
    }
    .await;
    group.stop().await;
    save(
        &out.join("stopped.json"),
        &serde_json::to_value(group.status()).map_err(|e| e.to_string())?,
    )?;
    match result {
        Ok(mut report) => {
            report["workers_stopped"] = json!(true);
            report["worker_stop_error"] = Value::Null;
            save(&out.join("answers.json"), &report)?;
            println!(
                "{}",
                json!({"collected":report["answers"].as_array().unwrap().len(),"out":out})
            );
            Ok(())
        }
        Err(error) => {
            save(
                &out.join("failure.json"),
                &json!({"error":error,"workers_stopped":true}),
            )?;
            Err(error)
        }
    }
}
