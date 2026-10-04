//! Manual performance acceptance over retained frame measurements.
use serde::Deserialize;
use std::{io::BufRead, path::Path};

#[derive(Deserialize)]
struct Row {
    schema: String,
    adapter: String,
    scene_time: f64,
    frame_work_ms: f64,
    #[serde(default)]
    frame_interval_ms: f64,
    #[serde(default)]
    schedule_dropped_seconds: f64,
    #[serde(default)]
    stress_casts: Option<u64>,
    #[serde(default)]
    spell_casts: Option<u64>,
    #[serde(default)]
    player_position: Option<[f64; 3]>,
    #[serde(default)]
    respawn_generation_max: Option<u64>,
}
fn percentile(values: &mut [f64], fraction: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len() - 1) as f64 * fraction).ceil() as usize]
}
pub fn check(path: &Path) -> Result<(), String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
        return Err("Frame profile exceeds 64 MiB".into());
    }
    let mut rows = Vec::new();
    for line in std::io::BufReader::new(file).lines() {
        if rows.len() >= 100_000 {
            return Err("Frame profile exceeds 100000 rows".into());
        }
        let row: Row =
            serde_json::from_str(&line.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if row.schema != "openagents.verse.frame-profile.v1"
            || !row.scene_time.is_finite()
            || [
                row.frame_work_ms,
                row.frame_interval_ms,
                row.schedule_dropped_seconds,
            ]
            .into_iter()
            .any(|v| !v.is_finite() || v < 0.)
            || rows.last().is_some_and(|old: &Row| {
                old.adapter != row.adapter || old.scene_time > row.scene_time
            })
        {
            return Err("Invalid frame profile".into());
        }
        rows.push(row);
    }
    let first = rows.first().ok_or("Frame profile is empty")?;
    let last = rows.last().unwrap();
    let duration = last.scene_time - first.scene_time;
    if rows.len() < 1200
        || duration < 45.
        || last.stress_casts.is_some_and(|n| n < 20 || duration < 90.)
    {
        return Err("Frame profile lacks sustained combat or movement/fireball evidence".into());
    }
    if last.stress_casts.is_some() {
        let start = first
            .player_position
            .ok_or("Stress profile lacks player positions")?;
        let moved = rows.iter().filter_map(|r| r.player_position).any(|p| {
            p.iter()
                .zip(start)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                > 1.
        });
        if !moved
            || last.spell_casts.is_none_or(|n| n < 20)
            || last.respawn_generation_max.is_none_or(|n| n == 0)
        {
            return Err(
                "Stress profile lacks movement, completed fireballs, or respawn evidence".into(),
            );
        }
    }
    let mut work: Vec<_> = rows
        .iter()
        .filter(|r| r.scene_time >= first.scene_time + 1.)
        .map(|r| r.frame_work_ms)
        .collect();
    let mut intervals: Vec<_> = rows
        .iter()
        .filter(|r| r.scene_time >= first.scene_time + 1. && r.frame_interval_ms > 0.)
        .map(|r| r.frame_interval_ms)
        .collect();
    let p95 = percentile(&mut work, 0.95);
    let maximum = *work.last().unwrap();
    let delivered_p95 = (!intervals.is_empty()).then(|| percentile(&mut intervals, 0.95));
    let delivered_max = intervals.last().copied();
    let passed = p95 <= 1000. / 60.
        && maximum <= 50.
        && delivered_p95.is_none_or(|v| v <= 20.)
        && delivered_max.is_none_or(|v| v <= 50.)
        && last.schedule_dropped_seconds == 0.;
    println!(
        "{}",
        serde_json::json!({"schema":"openagents.verse.frame-budget.v1","adapter":first.adapter,"frames":work.len(),"duration_seconds":duration,"work_p95_ms":p95,"work_max_ms":maximum,"delivered_p95_ms":delivered_p95,"delivered_max_ms":delivered_max,"stress_casts":last.stress_casts,"dropped_seconds":last.schedule_dropped_seconds,"passed":passed})
    );
    if passed {
        Ok(())
    } else {
        Err("Frame profile exceeds the performance budget".into())
    }
}
