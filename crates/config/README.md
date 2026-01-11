# config

Project configuration management for OpenAgents, providing loading, validation, and defaults for project settings stored in `.openagents/project.json`.

## Overview

The config crate implements user stories **CONF-001 through CONF-033**, covering:

- Basic configuration loading and saving (CONF-001..005)
- Safety and healer configuration (CONF-010..013)
- Codex integration settings (CONF-020..024)
- Sandbox execution configuration (CONF-030..033)

Configuration is stored at `.openagents/project.json` relative to your project root. Missing fields are automatically filled with sensible defaults.

## Quick Start

```rust
use config::{ProjectConfig, load_config, save_config};

// Load existing configuration
let config = load_config("/path/to/project")?;
println!("Project: {}", config.project_id);

// Create new configuration with defaults
let config = ProjectConfig::new("my-project");
save_config("/path/to/project", &config)?;
```

## Configuration Structure

### ProjectConfig

The main configuration struct containing all project settings.

```rust
use config::ProjectConfig;

let config = ProjectConfig::new("openagents");

// Core settings
assert_eq!(config.version, 1);
assert_eq!(config.default_branch, "main");
assert_eq!(config.default_model, "x-ai/grok-4.1-fast:free");
assert_eq!(config.max_tasks_per_run, 3);
assert_eq!(config.max_runtime_minutes, 240);

// Safety settings
assert!(config.allow_push);
assert!(!config.allow_force_push);

// Nested configurations
assert!(config.codex_code.enabled);
assert!(config.sandbox.enabled);
assert!(config.healer.enabled);
```

### Top-Level Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `version` | `u32` | `1` | Config schema version |
| `projectId` | `String` | *required* | Unique project identifier |
| `defaultBranch` | `String` | `"main"` | Default git branch |
| `workBranch` | `Option<String>` | `None` | Work branch if different |
| `defaultModel` | `String` | `"x-ai/grok-4.1-fast:free"` | Default LLM model |
| `rootDir` | `String` | `"."` | Project root directory |
| `allowPush` | `bool` | `true` | Allow git push |
| `allowForcePush` | `bool` | `false` | Allow git force push |
| `maxTasksPerRun` | `u32` | `3` | Max tasks per autopilot run |
| `maxRuntimeMinutes` | `u32` | `240` | Max runtime (4 hours) |
| `idPrefix` | `String` | `"oa"` | Task ID prefix |
| `sessionDir` | `String` | `".openagents/sessions"` | Session storage |
| `runLogDir` | `String` | `".openagents/run-logs"` | Run log storage |

## Codex Configuration

Settings for Codex integration.

```rust
use config::{CodexCodeConfig, PermissionMode};

let mut config = CodexCodeConfig::default();

config.enabled = true;
config.prefer_for_complex_tasks = true;
config.max_turns_per_subtask = 300;
config.permission_mode = PermissionMode::BypassPermissions;
config.fallback_to_minimal = true;
```

### Permission Modes

```rust
use config::PermissionMode;

// Available modes
let modes = vec![
    PermissionMode::Default,            // Standard behavior
    PermissionMode::AcceptEdits,        // Auto-accept suggested edits
    PermissionMode::BypassPermissions,  // Skip all permission checks (default)
    PermissionMode::Plan,               // Plan-only mode (no execution)
    PermissionMode::DontAsk,            // Don't ask for permissions
];
```

### JSON Configuration

```json
{
  "codexCode": {
    "enabled": true,
    "preferForComplexTasks": true,
    "maxTurnsPerSubtask": 300,
    "permissionMode": "bypassPermissions",
    "fallbackToMinimal": true
  }
}
```

## Sandbox Configuration

Sandboxing settings for secure code execution.

```rust
use config::{SandboxConfig, SandboxBackend};

let mut config = SandboxConfig::default();

config.enabled = true;
config.backend = SandboxBackend::Auto;
config.image = Some("openagents/sandbox:latest".into());
config.memory_limit = Some("8G".into());
config.cpu_limit = Some(4.0);
config.timeout_ms = 300_000; // 5 minutes
```

### Sandbox Backends

```rust
use config::SandboxBackend;

// Available backends
let backends = vec![
    SandboxBackend::Auto,             // Auto-detect best backend
    SandboxBackend::MacosContainer,   // macOS containerization
    SandboxBackend::Docker,           // Docker containers
    SandboxBackend::Seatbelt,         // macOS Seatbelt
    SandboxBackend::None,             // No sandboxing
];
```

### JSON Configuration

```json
{
  "sandbox": {
    "enabled": true,
    "backend": "auto",
    "image": "openagents/sandbox:latest",
    "memoryLimit": "8G",
    "cpuLimit": 4.0,
    "timeoutMs": 300000
  }
}
```

## Healer Configuration

Self-healing settings for automatic error recovery.

```rust
use config::{HealerConfig, HealerMode, HealerScenarioConfig};

let mut config = HealerConfig::default();

config.enabled = true;
config.max_invocations_per_session = 2;
config.max_invocations_per_subtask = 1;
config.mode = HealerMode::Conservative;
config.stuck_threshold_hours = 2;

// Scenario-specific healing
config.scenarios.on_init_failure = true;
config.scenarios.on_verification_failure = true;
config.scenarios.on_subtask_failure = true;
config.scenarios.on_runtime_error = true;
config.scenarios.on_stuck_subtask = false;
```

### Healer Modes

```rust
use config::HealerMode;

// Aggressiveness levels
let modes = vec![
    HealerMode::Conservative,  // Fewer interventions (default)
    HealerMode::Moderate,      // Balanced approach
    HealerMode::Aggressive,    // More interventions
];
```

### JSON Configuration

```json
{
  "healer": {
    "enabled": true,
    "maxInvocationsPerSession": 2,
    "maxInvocationsPerSubtask": 1,
    "mode": "conservative",
    "stuckThresholdHours": 2,
    "scenarios": {
      "onInitFailure": true,
      "onVerificationFailure": true,
      "onSubtaskFailure": true,
      "onRuntimeError": true,
      "onStuckSubtask": false
    }
  }
}
```

## Parallel Execution

Settings for running multiple agents concurrently.

```rust
use config::{ParallelExecutionConfig, MergeStrategy};

let mut config = ParallelExecutionConfig::default();

config.enabled = false;  // Disabled by default
config.max_agents = 2;
config.per_agent_memory_mb = 4096;
config.host_memory_reserve_mb = 6144;
config.worktree_timeout = 30 * 60 * 1000;      // 30 minutes
config.install_timeout_ms = 15 * 60 * 1000;     // 15 minutes
config.install_args = vec!["--frozen-lockfile".into()];
config.merge_strategy = MergeStrategy::Auto;
config.merge_threshold = 4;
config.pr_threshold = 50;
```

### Merge Strategies

```rust
use config::MergeStrategy;

let strategies = vec![
    MergeStrategy::Auto,        // Auto-detect best strategy
    MergeStrategy::Sequential,  // Sequential merging
    MergeStrategy::Parallel,    // Parallel merging
];
```

### JSON Configuration

```json
{
  "parallelExecution": {
    "enabled": false,
    "maxAgents": 2,
    "perAgentMemoryMb": 4096,
    "hostMemoryReserveMb": 6144,
    "worktreeTimeout": 1800000,
    "installTimeoutMs": 900000,
    "installArgs": ["--frozen-lockfile"],
    "mergeStrategy": "auto",
    "mergeThreshold": 4,
    "prThreshold": 50
  }
}
```

## Additional Configurations

### Trajectory Recording

```rust
use config::TrajectoryConfig;

let config = TrajectoryConfig {
    enabled: true,
    output_dir: ".openagents/trajectories".into(),
};
```

JSON:
```json
{
  "trajectory": {
    "enabled": true,
    "outputDir": ".openagents/trajectories"
  }
}
```

### Reflexion (Self-Improvement)

```rust
use config::ReflexionConfig;

let config = ReflexionConfig {
    enabled: true,
    max_iterations: 3,
};
```

JSON:
```json
{
  "reflexion": {
    "enabled": true,
    "maxIterations": 3
  }
}
```

### Failure Cleanup

```rust
use config::FailureCleanupConfig;

let config = FailureCleanupConfig {
    revert_tracked_files: true,
    delete_untracked_files: false,
};
```

JSON:
```json
{
  "failureCleanup": {
    "revertTrackedFiles": true,
    "deleteUntrackedFiles": false
  }
}
```

### Terminal-Bench Configuration

```rust
use config::{TBenchConfig, LearningConfig};

let config = TBenchConfig {
    default_model: "fm".into(),
    default_suite: "docs/tb-tasks/fm-mini-suite.json".into(),
    default_timeout: 3600,
    default_max_turns: 300,
    default_learning: LearningConfig {
        skills: true,
        memory: true,
        reflexion: true,
        learn: true,
    },
};
```

JSON:
```json
{
  "tbench": {
    "defaultModel": "fm",
    "defaultSuite": "docs/tb-tasks/fm-mini-suite.json",
    "defaultTimeout": 3600,
    "defaultMaxTurns": 300,
    "defaultLearning": {
      "skills": true,
      "memory": true,
      "reflexion": true,
      "learn": true
    }
  }
}
```

## Loading and Saving

### Basic Operations

```rust
use config::{load_config, save_config, init_config, has_config};

// Check if config exists
if !has_config("/path/to/project") {
    // Initialize new config
    let config = init_config("/path/to/project", "my-project")?;
}

// Load existing config
let config = load_config("/path/to/project")?;

// Modify and save
let mut config = config;
config.max_tasks_per_run = 5;
save_config("/path/to/project", &config)?;
```

### Optional Loading

```rust
use config::load_config_optional;

// Returns None if config doesn't exist (no error)
if let Some(config) = load_config_optional("/path/to/project")? {
    println!("Found config: {}", config.project_id);
} else {
    println!("No config found, using defaults");
}
```

### Merging with Defaults

Useful for CLI overrides:

```rust
use config::merge_with_defaults;

// Partial JSON from CLI flags
let partial = r#"{
  "defaultBranch": "develop",
  "allowPush": false,
  "codexCode": {
    "maxTurnsPerSubtask": 100
  }
}"#;

let config = merge_with_defaults(partial, "my-project")?;

assert_eq!(config.default_branch, "develop");
assert!(!config.allow_push);
assert_eq!(config.codex_code.max_turns_per_subtask, 100);
// Defaults still applied
assert!(config.codex_code.enabled);
```

## Validation

Configurations are validated automatically on load and save:

```rust
use config::{ProjectConfig, save_config, ConfigError};

let mut config = ProjectConfig::new("test");

// Invalid: empty project ID
config.project_id = "".into();
assert!(matches!(
    save_config("/tmp", &config),
    Err(ConfigError::ValidationError(_))
));

// Invalid: zero max tasks
config.project_id = "test".into();
config.max_tasks_per_run = 0;
assert!(matches!(
    save_config("/tmp", &config),
    Err(ConfigError::ValidationError(_))
));
```

### Validation Rules

- `projectId`: Must not be empty
- `defaultBranch`: Must not be empty
- `idPrefix`: Must not be empty
- `maxTasksPerRun`: Must be positive
- `maxRuntimeMinutes`: Must be positive
- `codexCode.maxTurnsPerSubtask`: Must be positive
- `sandbox.timeoutMs`: Must be positive
- `parallelExecution.maxAgents`: Must be positive if enabled

## Test Commands

Configuration supports custom test, typecheck, and build commands:

```rust
let mut config = ProjectConfig::new("rust-project");

config.typecheck_commands = vec!["cargo check".into()];
config.test_commands = vec!["cargo test".into()];
config.sandbox_test_commands = vec!["cargo test --features sandbox".into()];
config.e2e_commands = vec!["cargo test --test e2e".into()];
```

JSON:
```json
{
  "typecheckCommands": ["cargo check"],
  "testCommands": ["cargo test"],
  "sandboxTestCommands": ["cargo test --features sandbox"],
  "e2eCommands": ["cargo test --test e2e"]
}
```

## File Location

Configuration is always stored at:
```
<project_root>/.openagents/project.json
```

The `.openagents` directory is created automatically when saving.

```rust
use config::config_path;

let path = config_path("/home/user/myproject");
assert_eq!(
    path.to_str().unwrap(),
    "/home/user/myproject/.openagents/project.json"
);
```

## Error Handling

```rust
use config::{ConfigError, load_config};

match load_config("/path/to/project") {
    Ok(config) => println!("Loaded: {}", config.project_id),
    Err(ConfigError::NotFound(path)) => {
        eprintln!("Config not found: {}", path.display());
    }
    Err(ConfigError::ParseError(msg)) => {
        eprintln!("Invalid JSON: {}", msg);
    }
    Err(ConfigError::ValidationError(msg)) => {
        eprintln!("Validation failed: {}", msg);
    }
    Err(ConfigError::ReadError(msg)) => {
        eprintln!("Read error: {}", msg);
    }
    Err(ConfigError::WriteError(msg)) => {
        eprintln!("Write error: {}", msg);
    }
}
```

## Example Configuration

Complete example showing all major sections:

```json
{
  "version": 1,
  "projectId": "openagents",
  "defaultBranch": "main",
  "defaultModel": "x-ai/grok-4.1-fast:free",
  "rootDir": ".",
  "allowPush": true,
  "allowForcePush": false,
  "maxTasksPerRun": 3,
  "maxRuntimeMinutes": 240,
  "idPrefix": "oa",
  "sessionDir": ".openagents/sessions",
  "runLogDir": ".openagents/run-logs",

  "typecheckCommands": ["cargo check"],
  "testCommands": ["cargo test"],
  "sandboxTestCommands": [],
  "e2eCommands": [],

  "codexCode": {
    "enabled": true,
    "preferForComplexTasks": true,
    "maxTurnsPerSubtask": 300,
    "permissionMode": "bypassPermissions",
    "fallbackToMinimal": true
  },

  "sandbox": {
    "enabled": true,
    "backend": "auto",
    "timeoutMs": 300000
  },

  "healer": {
    "enabled": true,
    "maxInvocationsPerSession": 2,
    "maxInvocationsPerSubtask": 1,
    "mode": "conservative",
    "stuckThresholdHours": 2,
    "scenarios": {
      "onInitFailure": true,
      "onVerificationFailure": true,
      "onSubtaskFailure": true,
      "onRuntimeError": true,
      "onStuckSubtask": false
    }
  },

  "parallelExecution": {
    "enabled": false,
    "maxAgents": 2,
    "perAgentMemoryMb": 4096,
    "hostMemoryReserveMb": 6144,
    "worktreeTimeout": 1800000,
    "installTimeoutMs": 900000,
    "installArgs": ["--frozen-lockfile"],
    "mergeStrategy": "auto",
    "mergeThreshold": 4,
    "prThreshold": 50
  },

  "trajectory": {
    "enabled": true,
    "outputDir": ".openagents/trajectories"
  },

  "reflexion": {
    "enabled": true,
    "maxIterations": 3
  },

  "failureCleanup": {
    "revertTrackedFiles": true,
    "deleteUntrackedFiles": false
  },

  "tbench": {
    "defaultModel": "fm",
    "defaultSuite": "docs/tb-tasks/fm-mini-suite.json",
    "defaultTimeout": 3600,
    "defaultMaxTurns": 300,
    "defaultLearning": {
      "skills": true,
      "memory": true,
      "reflexion": true,
      "learn": true
    }
  }
}
```

## Testing

The crate includes comprehensive tests:

```bash
# Run all tests
cargo test --package config

# Test specific functionality
cargo test --package config test_save_and_load_config
cargo test --package config test_validation
cargo test --package config test_merge_with_defaults
```

## Design Principles

1. **Defaults for Everything**: All fields have sensible defaults
2. **Validation on Load**: Invalid configs fail fast with clear errors
3. **Partial Updates**: Merge partial JSON with defaults for CLI overrides
4. **Backward Compatibility**: Version field allows future schema evolution
5. **Nested Structure**: Logical grouping of related settings
6. **Type Safety**: Enums for modes and backends prevent invalid values

## Related Modules

- `types.rs`: Configuration type definitions and defaults
- `loader.rs`: Loading, saving, and validation logic
- `lib.rs`: Public API re-exports

## Dependencies

- `serde`/`serde_json`: JSON serialization
- `thiserror`: Error handling
- `tempfile` (dev): Testing with temporary directories

## License

Same as the OpenAgents workspace.
