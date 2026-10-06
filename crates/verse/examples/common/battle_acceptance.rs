//! Capacity verdicts require observed workloads and declared budgets.
use serde_json::Value;

pub fn failures(report: &Value) -> Vec<String> {
    let mut failures = Vec::new();
    if report["status"] != "complete" {
        failures.push("Workload process did not complete".into());
    }
    let limit = |path: &str, maximum: f64, failures: &mut Vec<String>| {
        let value = report.pointer(path).and_then(Value::as_f64);
        if value.is_none_or(|value| !value.is_finite() || value < 0. || value > maximum) {
            failures.push(format!(
                "{path}: {value:?} exceeds {maximum}, or is unavailable"
            ));
        }
    };
    if report["mode"] == "authority" {
        limit(
            "/authority/measurements/steady/simulation_cpu_ms/p99",
            1000. / 30.,
            &mut failures,
        );
        if report["authority"]["minimum_live_hostiles"] != 40 {
            failures.push("Authority did not sustain forty hostiles".into());
        }
        if report["authority"]["accepted_battle_movement_per_player"]
            .as_array()
            .is_none_or(|players| {
                players.len() != 20 || players.iter().any(|moves| moves.as_u64().unwrap_or(0) < 10)
            })
        {
            failures.push("Authority lacks accepted battle movement for twenty players".into());
        }
        return failures;
    }
    if report["seconds"]
        .as_f64()
        .zip(report["server"]["workload_ticks"].as_f64())
        .is_none_or(|(seconds, ticks)| ticks < seconds * 30. * 0.98)
    {
        failures.push("Authority throughput fell below ninety-eight percent of 30 Hz".into());
    }
    limit(
        "/server/simulation/steady/p99_upper_bound_ms",
        1000. / 30.,
        &mut failures,
    );
    limit(
        "/render/measurements/steady/applied_snapshot_age_ms/p95",
        400.,
        &mut failures,
    );
    limit("/server/movement_expiry/total", 0., &mut failures);
    limit("/server/request_queue_peak", 128., &mut failures);
    limit("/server/writer_queue_peak", 2., &mut failures);
    limit("/server/held_reply_bytes_peak", 33554432., &mut failures);
    limit("/recovery/active_receipts", 128., &mut failures);
    limit("/recovery/retained_events", 512., &mut failures);
    if !report["server"]["failure"].is_null() {
        failures.push("The authority failed".into());
    }
    if report["recovery"]["characters_checked"] != 20 || report["recovery"]["live_hostiles"] != 40 {
        failures
            .push("Durable recovery did not preserve twenty characters and forty hostiles".into());
    }
    let mut operations = std::collections::BTreeMap::<String, u64>::new();
    let mut casts = std::collections::BTreeMap::<String, u64>::new();
    let players = report["players"].as_array();
    if players.is_none_or(|players| players.len() != 20) {
        failures.push("Twenty authenticated player records are required".into());
    }
    for player in players.into_iter().flatten() {
        let segments = player["segments"].as_array();
        if segments.is_none_or(|segments| segments.len() != 2) {
            failures.push(format!(
                "Player {} did not finish its reconnect",
                player["player"]
            ));
        }
        for segment in segments.into_iter().flatten() {
            if segment["status"] != "complete"
                || segment["maximum_players"] != 20
                || segment["battle_occupancy"]["samples"].as_u64().unwrap_or(0) == 0
                || segment["battle_occupancy"]["minimum_live_hostiles"] != 40
                || segment["observed_frame_snapshots"].as_u64().unwrap_or(0) == 0
                || segment["movement_inputs"].as_u64().unwrap_or(0) == 0
            {
                failures.push(format!(
                    "Player {} lacks sustained interval battle observations",
                    player["player"]
                ));
            }
            for (label, count) in segment["accepted_operations"]
                .as_object()
                .into_iter()
                .flatten()
            {
                *operations.entry(label.clone()).or_default() += count.as_u64().unwrap_or(0);
            }
            if segment["bound_frames"].as_u64().unwrap_or(0) < 10 {
                failures.push(format!(
                    "Player {} did not transmit ten native movement intervals",
                    player["player"]
                ));
            }
            let samples = segment["battle_occupancy"]["samples"].as_u64().unwrap_or(0);
            let framed = segment["battle_framed_snapshots"].as_u64().unwrap_or(0);
            if samples == 0 || framed as f64 / (samples as f64) < 0.8 {
                failures.push(format!("Player {} did not sustain interval movement for eighty percent of battle snapshots",player["player"]));
            }
            if report["native_sessions"].as_u64() == Some(20)
                && segment["movement_profile"] != "native_session_intervals"
            {
                failures.push(format!(
                    "Player {} did not use the declared native session",
                    player["player"]
                ));
            }
            if segment["movement_profile"] == "native_session_intervals" {
                for field in ["prediction_failures", "prediction_horizon_pauses"] {
                    if segment[field].as_u64() != Some(0) {
                        failures.push(format!(
                            "Player {} {field} is nonzero or unavailable",
                            player["player"]
                        ));
                    }
                }
            }
            if player["player"] == 0 || segment["movement_profile"] == "native_session_intervals" {
                for (field, maximum) in [("p95", 0.25), ("maximum", 1.)] {
                    let value = segment["measurements"]["steady"]["prediction_correction_meters"]
                        [field]
                        .as_f64();
                    if value.is_none_or(|v| !v.is_finite() || v > maximum) {
                        failures.push(format!("Player {} prediction {field} {value:?} exceeds {maximum} meters or is unavailable",player["player"]));
                    }
                }
            }
            if player["player"] == 0 || segment["movement_profile"] == "native_session_intervals" {
                for window in segment["windows"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|w| w["end_seconds"].as_f64().is_some_and(|end| end >= 30.))
                {
                    for (field, maximum) in [("p95", 0.25), ("maximum", 1.)] {
                        let value =
                            window["measurements"]["steady"]["prediction_correction_meters"][field]
                                .as_f64();
                        if value.is_none_or(|v| !v.is_finite() || v > maximum) {
                            failures.push(format!("Player {} late prediction {field} {value:?} exceeds {maximum} meters or is unavailable",player["player"]));
                        }
                    }
                }
            }
            for (label, count) in segment["accepted_casts"].as_object().into_iter().flatten() {
                *casts.entry(label.clone()).or_default() += count.as_u64().unwrap_or(0);
            }
            if segment["confirmed_interval_steps"].as_u64().unwrap_or(0) < 240 {
                failures.push(format!(
                    "Player {} lacks two seconds of confirmed interval physics",
                    player["player"]
                ));
            }
        }
    }
    for operation in ["equipment", "quest_claim", "item_use", "respawn"] {
        if operations.get(operation).copied().unwrap_or(0) == 0 {
            failures.push(format!("No accepted {operation} operation"));
        }
    }
    for ability in ["Fireball", "Web", "Thunderwave"] {
        if casts.get(ability).copied().unwrap_or(0) == 0 {
            failures.push(format!("The workload has no accepted {ability} casts"));
        }
    }
    limit("/recovery/checkpoint_bytes", 1048576., &mut failures);
    if report["route"]["connections"] != 40
        || report["route"]["refused_connections"] != 0
        || report["server"]["admission"]["active_peak"] != 20
    {
        failures.push(
            "The delayed route did not admit forty sessions and twenty concurrent players".into(),
        );
    }
    let windows = report["disconnect_windows_seconds"].as_array();
    if report["route"]["omitted_error_details"] != 0 {
        failures.push("Delayed route error evidence was omitted".into());
    }
    for error in report["route"]["error_details"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let elapsed = error["elapsed_seconds"].as_f64();
        let at_disconnect = elapsed.is_some_and(|at| {
            windows
                .into_iter()
                .flatten()
                .filter_map(Value::as_f64)
                .any(|end| (at - end).abs() < 3.)
        });
        let closure = error["type"] == "ConnectionResetError" || error["type"] == "BrokenPipeError";
        if !closure || !at_disconnect {
            failures.push(format!("Unclassified delayed route error: {error}"));
        }
    }
    for window in report["render"]["windows"].as_array().into_iter().flatten() {
        if window["end_seconds"].as_f64().is_some_and(|at| at > 35.) {
            let age = window["measurements"]["steady"]["applied_snapshot_age_ms"]["p95"].as_f64();
            if age.is_none_or(|age| age > 400.) {
                failures.push("A lifetime window exceeded the snapshot freshness budget".into());
            }
        }
    }
    let rss = report["render"]["rss"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| row["seconds"].as_f64().is_some_and(|at| at >= 30.))
        .filter_map(|row| row["bytes"].as_u64())
        .collect::<Vec<_>>();
    if report["seconds"].as_u64().is_some_and(|s| s >= 600) {
        if rss.len() < 60
            || rss
                .iter()
                .max()
                .zip(rss.first())
                .is_none_or(|(last, first)| last.saturating_sub(*first) > 67108864)
        {
            failures.push("The soak lacks bounded steady resident memory evidence".into());
        }
    }
    if report["mode"] == "combined" {
        limit(
            "/render/measurements/steady/frame_cpu_ms/p95",
            1000. / 60.,
            &mut failures,
        );
        limit(
            "/render/measurements/steady/gpu_scene_ms/p95",
            1000. / 60.,
            &mut failures,
        );
    }
    failures
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_successful_exit_and_fast_empty_profiles_do_not_pass_capacity() {
        let report = serde_json::json!({"status":"complete","mode":"combined",
            "server":{"simulation":{"steady":{"p99_upper_bound_ms":1.}}},
            "render":{"measurements":{"steady":{"draw_cpu_ms":{"p95":1.},"gpu_scene_ms":{"p95":1.},"applied_snapshot_age_ms":{"p95":1.}}}}});
        let refused = failures(&report);
        assert!(
            refused
                .iter()
                .any(|failure| failure.contains("authenticated"))
        );
        assert!(
            refused
                .iter()
                .any(|failure| failure.contains("Durable recovery"))
        );
    }
    #[test]
    fn missing_and_over_budget_authority_timings_refuse_a_full_population() {
        let mut report = serde_json::json!({"status":"complete","mode":"authority","authority":{"minimum_live_hostiles":40}});
        assert!(!failures(&report).is_empty());
        report["authority"]["measurements"] =
            serde_json::json!({"steady":{"simulation_cpu_ms":{"p99":34.}}});
        assert!(!failures(&report).is_empty());
        report["authority"]["measurements"]["steady"]["simulation_cpu_ms"]["p99"] =
            serde_json::json!(16.);
        assert!(!failures(&report).is_empty());
        report["authority"]["accepted_battle_movement_per_player"] =
            serde_json::json!(vec![100; 20]);
        assert!(failures(&report).is_empty());
    }
    #[test]
    fn fast_profiles_do_not_hide_missing_actions_or_route_failures() {
        let segment = serde_json::json!({"status":"complete","maximum_players":20,"battle_occupancy":{"samples":100,"minimum_live_hostiles":40},"observed_frame_snapshots":100,"battle_framed_snapshots":100,"movement_inputs":100,"bound_frames":30,"confirmed_interval_steps":360,"accepted_operations":{"equipment":1,"quest_claim":1,"item_use":1,"respawn":1},"accepted_casts":{"Fireball":1,"Web":1,"Thunderwave":1},"measurements":{"steady":{"prediction_correction_meters":{"p95":0.01,"maximum":0.1}}}});
        let mut report = serde_json::json!({"status":"complete","mode":"network","seconds":60,"server":{"movement_expiry":{"total":0},"ticks":1800,"workload_ticks":1800,"request_queue_peak":20,"writer_queue_peak":2,"held_reply_bytes_peak":1000000,"failure":null,"simulation":{"steady":{"p99_upper_bound_ms":16.}},"admission":{"active_peak":20}},"render":{"measurements":{"steady":{"applied_snapshot_age_ms":{"p95":100.}}}},"recovery":{"active_receipts":32,"retained_events":512,"characters_checked":20,"live_hostiles":40,"checkpoint_bytes":300000},"route":{"connections":40,"refused_connections":0,"omitted_error_details":0,"error_details":[]},"disconnect_windows_seconds":[30.,60.],"players":(0..20).map(|player|serde_json::json!({"player":player,"segments":[segment.clone(),segment.clone()]})).collect::<Vec<_>>()});
        assert!(failures(&report).is_empty());
        report["players"][7]["segments"][0]["prediction_failures"] = serde_json::json!(0);
        report["players"][7]["segments"][0]["prediction_horizon_pauses"] = serde_json::json!(0);
        report["players"][7]["segments"][0]["movement_profile"] =
            serde_json::json!("native_session_intervals");
        report["players"][7]["segments"][0]["measurements"]["steady"]["prediction_correction_meters"]
            ["p95"] = serde_json::json!(0.3);
        assert!(
            failures(&report)
                .iter()
                .any(|f| f.contains("Player 7 prediction p95"))
        );
        report["players"][7]["segments"][0]["measurements"]["steady"]["prediction_correction_meters"]
            ["p95"] = serde_json::json!(0.01);
        assert!(failures(&report).is_empty());
        report["players"][7]["segments"][0]["prediction_failures"] = serde_json::json!(1);
        assert!(
            failures(&report)
                .iter()
                .any(|f| f.contains("Player 7 prediction_failures"))
        );
        report["players"][7]["segments"][0]["prediction_failures"] = serde_json::json!(0);
        report["native_sessions"] = serde_json::json!(20);
        assert!(
            failures(&report)
                .iter()
                .any(|f| f.contains("declared native session"))
        );
        report.as_object_mut().unwrap().remove("native_sessions");
        report["server"]["movement_expiry"]["total"] = serde_json::json!(13);
        assert!(
            failures(&report)
                .iter()
                .any(|f| f.contains("/server/movement_expiry/total"))
        );
        report["server"]
            .as_object_mut()
            .unwrap()
            .remove("movement_expiry");
        assert!(
            failures(&report)
                .iter()
                .any(|f| f.contains("/server/movement_expiry/total"))
        );
        report["server"]["movement_expiry"] = serde_json::json!({"total":0});
        assert!(failures(&report).is_empty());
        report["server"]["workload_ticks"] = serde_json::json!(1700);
        assert!(failures(&report).iter().any(|f| f.contains("throughput")));
        report["server"]["workload_ticks"] = serde_json::json!(1800);
        report["players"][0]["segments"][0]["windows"] = serde_json::json!([
            {"end_seconds":31.,"measurements":{"steady":{"prediction_correction_meters":{"p95":0.3,"maximum":0.8}}}}
        ]);
        assert!(
            failures(&report)
                .iter()
                .any(|f| f.contains("late prediction p95"))
        );
        report["players"][0]["segments"][0]["windows"][0]["measurements"]["steady"]["prediction_correction_meters"]
            ["p95"] = serde_json::json!(0.1);
        report["players"][0]["segments"][0]["windows"][0]["measurements"]["steady"]["prediction_correction_meters"]
            ["maximum"] = serde_json::json!(1.1);
        assert!(
            failures(&report)
                .iter()
                .any(|f| f.contains("late prediction maximum"))
        );
        report["players"][0]["segments"][0]["windows"] = serde_json::json!([]);
        for player in report["players"].as_array_mut().unwrap() {
            for segment in player["segments"].as_array_mut().unwrap() {
                segment["accepted_operations"]["quest_claim"] = serde_json::json!(0);
            }
        }
        assert!(
            failures(&report)
                .iter()
                .any(|failure| failure.contains("quest_claim"))
        );
        report["route"]["error_details"] =
            serde_json::json!([{"type":"ConnectionResetError","elapsed_seconds":15.}]);
        assert!(
            failures(&report)
                .iter()
                .any(|failure| failure.contains("Unclassified"))
        );
        report["route"]["error_details"][0]["elapsed_seconds"] = serde_json::json!(30.1);
        assert!(
            !failures(&report)
                .iter()
                .any(|failure| failure.contains("Unclassified"))
        );
    }
}
