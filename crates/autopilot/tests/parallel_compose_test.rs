//! Tests for parallel autopilot docker compose configuration

use std::fs;
use std::path::PathBuf;

fn find_project_root() -> PathBuf {
    let mut current = std::env::current_dir().expect("current dir");

    loop {
        let compose_path = current.join("docker/autopilot/docker-compose.yml");
        if compose_path.exists() {
            return current;
        }

        if !current.pop() {
            break;
        }
    }

    panic!("project root with docker/autopilot/docker-compose.yml not found");
}

fn compose_path() -> PathBuf {
    find_project_root().join("docker/autopilot/docker-compose.yml")
}

#[test]
fn test_compose_shares_issue_db() {
    let compose = fs::read_to_string(compose_path()).expect("compose file should exist");

    assert!(
        compose.contains("../../autopilot.db:/shared/autopilot.db:rw"),
        "compose should mount shared issue database"
    );
    assert!(
        compose.contains("ISSUES_DB=/shared/autopilot.db"),
        "compose should pass shared DB path to agents"
    );
}

#[test]
fn test_compose_per_agent_resource_overrides() {
    let compose = fs::read_to_string(compose_path()).expect("compose file should exist");

    for id in ["001", "005", "010"] {
        let memory_var = format!("AGENT_MEMORY_{}", id);
        let cpu_var = format!("AGENT_CPUS_{}", id);

        assert!(
            compose.contains(&memory_var),
            "compose should reference {} for memory limits",
            memory_var
        );
        assert!(
            compose.contains(&cpu_var),
            "compose should reference {} for cpu limits",
            cpu_var
        );
    }
}
