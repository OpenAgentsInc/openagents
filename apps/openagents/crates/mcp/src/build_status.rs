//! OpenAgents #11971: `build_status`, the coordinator's view of agent builds.
//!
//! Agent chats build through the gate in `coder_lease::agent_builds`: heavy
//! `cargo` commands wait for one of a limited number of build slots and
//! don't start below the disk floor. This tool reads the lease table on this
//! computer and reports the builds running and waiting, the limit, and the
//! free disk. With `limit`, it sets the limit for builds that start from
//! then on. Like `changed_paths`, it reads local files instead of the
//! engine, because builds are this computer's, not the workspace's.

use std::path::Path;

use serde_json::{Value, json};

use crate::tools::ToolDef;

pub const NAME: &str = "build_status";

pub fn tool() -> ToolDef {
    ToolDef {
        name: NAME,
        description: "Agent builds on this computer: how many cargo builds are running and waiting, the most that may run at once, free disk, and the floor below which builds don't start. Pass limit to change how many may run at once (default 5); builds over the limit wait on their own. Check it before you start chats that build.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Set how many agent builds may run at once."
                }
            }
        }),
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    limit: Option<u64>,
}

/// Run the tool against this computer's lease table.
pub fn call(args: Value) -> Result<Value, String> {
    let root = coder_lease::root_from_env()?;
    let disk = coder_lease::agent_builds::Gate::from_env()
        .map_or_else(|| root.clone(), |gate| gate.targets);
    let floor = coder_lease::agent_builds::floor_bytes(&|name| std::env::var(name).ok());
    call_at(&root, &disk, floor, args, &coder_lease::free_disk)
}

fn call_at(
    root: &Path,
    disk: &Path,
    floor: u64,
    args: Value,
    free_disk: &dyn Fn(&Path) -> std::io::Result<u64>,
) -> Result<Value, String> {
    let args: Args = if args.is_null() {
        Args { limit: None }
    } else {
        serde_json::from_value(args).map_err(|e| format!("invalid arguments: {e}"))?
    };
    if let Some(limit) = args.limit {
        coder_lease::agent_builds::set_limit(root, limit)
            .map_err(|e| format!("couldn't set the build limit: {e}"))?;
    }
    let status = coder_lease::agent_builds::status_with(root, disk, floor, free_disk)
        .map_err(|e| format!("couldn't read the builds on this computer: {e}"))?;
    let gib =
        |bytes: u64| (bytes as f64 / coder_lease::agent_builds::GIB as f64 * 10.0).round() / 10.0;
    Ok(json!({
        "running": status.running,
        "waiting": status.waiting,
        "limit": status.limit,
        "freeGib": status.free_bytes.map(gib),
        "floorGib": gib(status.floor_bytes),
        "belowFloor": status.below_floor,
        "runningFor": status.holders,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_lease::agent_builds::GIB;

    #[test]
    fn reports_builds_and_sets_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("leases");
        let free = |_: &Path| Ok(63 * GIB);
        let report = call_at(&root, dir.path(), 30 * GIB, Value::Null, &free).unwrap();
        assert_eq!(report["running"], 0);
        assert_eq!(report["waiting"], 0);
        assert_eq!(report["limit"], 5);
        assert_eq!(report["freeGib"], 63.0);
        assert_eq!(report["floorGib"], 30.0);
        assert_eq!(report["belowFloor"], false);
        let report = call_at(&root, dir.path(), 30 * GIB, json!({"limit": 3}), &free).unwrap();
        assert_eq!(report["limit"], 3);
        let low = |_: &Path| Ok(12 * GIB);
        let report = call_at(&root, dir.path(), 30 * GIB, json!({}), &low).unwrap();
        assert_eq!(
            (report["limit"].clone(), report["belowFloor"].clone()),
            (json!(3), json!(true))
        );
        assert!(call_at(&root, dir.path(), 0, json!({"limit": 0}), &free).is_err());
        assert!(call_at(&root, dir.path(), 0, json!({"slots": 2}), &free).is_err());
    }
}
