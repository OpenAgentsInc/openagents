# Pylon Compute Mix Integration Test Log

**Date:** 2025-12-28 11:02 PST
**Test:** Pylon compute mix CLI and autopilot integration

## Summary

Thorough testing of the pylon compute mix integration into the autopilot startup flow and CLI.

## Bug Fix: Llama.cpp Model Detection

**Issue:** The initial implementation only extracted models from Ollama, but not from Llama.cpp/GPT-OSS or other OpenAI-compatible backends (apple_fm, fm-bridge). This caused the compute mix to show Llama.cpp as available but with no models, even when gpt-oss was running.

**Fix:** Added `get_openai_compatible_models()` function that fetches models from backends using the OpenAI-compatible `/v1/models` endpoint. Both `crates/pylon/src/cli/compute.rs` and `crates/autopilot/src/pylon_integration.rs` were updated to call this function for llama.cpp, apple_fm, and fm-bridge backends.

**Files Modified:**
- `crates/pylon/src/cli/compute.rs` - Added OpenAI-compatible model detection
- `crates/autopilot/src/pylon_integration.rs` - Added OpenAI-compatible model detection

**New Tests Added:**
- `test_detect_local_backends_names` - Verify all 4 backend names are present
- `test_check_backend_unavailable` - Verify unavailable backends return empty models
- `test_check_backend_model_detection_llamacpp` - Ensure no panic on llamacpp detection
- `test_detect_cloud_providers` - Verify cloud provider detection

## Test Results

### 1. CLI Compute Command - Human Readable Output

```
$ ./target/debug/openagents pylon compute

Compute Mix
===========

Local Pylon:
  Status: Stopped
  Run 'openagents pylon start' to start

Local Backends:
  [OK] ollama (http://localhost:11434) - llama3.2:latest, gpt-oss:120b-cloud, gpt-oss:20b, glm-4.6:cloud, nomic-embed-text:latest
  [--] apple_fm (not running)
  [--] llamacpp (not running)
  [--] fm-bridge (not running)

Cloud Providers:
  None configured

Swarm Providers (NIP-89):
  None discovered

Summary:
  1 local backend(s), 0 cloud provider(s), 0 swarm provider(s)
```

**Result:** PASS - Correctly detects Ollama with 5 models, shows other backends as not running

### 2. CLI Compute Command - JSON Output

```json
$ ./target/debug/openagents pylon compute --json

{
  "pylon": {
    "running": false,
    "status": {
      "uptime_secs": 2422,
      "jobs_completed": 22,
      "provider_active": true,
      "host_active": false
    }
  },
  "local_backends": [
    {
      "name": "ollama",
      "available": true,
      "endpoint": "http://localhost:11434",
      "models": [
        "llama3.2:latest",
        "gpt-oss:120b-cloud",
        "gpt-oss:20b",
        "glm-4.6:cloud",
        "nomic-embed-text:latest"
      ]
    },
    {
      "name": "apple_fm",
      "available": false,
      "endpoint": "http://localhost:11435",
      "models": []
    },
    {
      "name": "llamacpp",
      "available": false,
      "endpoint": "http://localhost:8080",
      "models": []
    },
    {
      "name": "fm-bridge",
      "available": false,
      "endpoint": "http://localhost:8081",
      "models": []
    }
  ],
  "cloud_providers": [],
  "swarm_providers": []
}
```

**Result:** PASS - Valid JSON with all expected fields

### 3. CLI Help Output

```
$ ./target/debug/openagents pylon compute --help

Show compute mix (all available compute options)

Usage: openagents pylon compute [OPTIONS]

Options:
      --json     Output as JSON
  -v, --verbose  Enable verbose logging
  -h, --help     Print help
```

**Result:** PASS - Help shows expected options

### 4. Pylon Command Shows Compute Subcommand

```
$ ./target/debug/openagents pylon --help

Pylon commands (NIP-90 compute provider)

Usage: openagents pylon [OPTIONS] <COMMAND>

Commands:
  init      Initialize pylon identity
  start     Start the pylon daemon
  stop      Stop the pylon daemon
  status    Show daemon status
  doctor    Run diagnostics
  agent     Manage agents (host mode)
  earnings  View earnings (provider mode)
  compute   Show compute mix (all available compute options)
  help      Print this message or the help of the given subcommand(s)
```

**Result:** PASS - compute command listed in help

### 5. Pylon Status Command

```
$ ./target/debug/openagents pylon status

[2025-12-28T17:03:21.783356Z] INFO compute::backends: Detected Ollama backend at localhost:11434
[2025-12-28T17:03:21.785194Z] INFO pylon::provider: Detected backends: ollama
Pylon Status
============

Daemon: Stopped

  Run 'pylon start' to start the daemon.

Identity:
  Configured

Backends:
  Available: ollama (default)

Relays:
  wss://relay.damus.io
  wss://nos.lol
  wss://relay.nostr.band
```

**Result:** PASS - Status command works and shows backend detection

### 6. CLI Integration Tests

```
$ cargo test --test cli_integration

running 40 tests
test test_pylon_compute_help ... ok
test test_pylon_subcommands_listed ... ok
... (all 40 tests)

test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Result:** PASS - All 40 CLI integration tests pass

### 7. Pylon Crate Tests

```
$ cargo test -p pylon

running 23 tests
test config::tests::test_default_config ... ok
test daemon::control::tests::test_command_serialization ... ok
... (all 23 tests)

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Result:** PASS - All 23 pylon tests pass

### 8. Autopilot Crate Tests

```
$ cargo test -p autopilot

running 20 tests
test pylon_integration::tests::test_check_pylon_running_doesnt_panic ... ok
test pylon_integration::tests::test_discover_swarm_providers_returns_empty ... ok
test pylon_integration::tests::test_detect_local_backends_returns_all ... ok
... (all 20 tests)

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Result:** PASS - All 20 autopilot tests pass including new pylon integration tests

## Files Modified/Created

| File | Type | Description |
|------|------|-------------|
| `crates/autopilot/src/preflight.rs` | Modified | Added PylonInfo, SwarmProvider, ComputeMix structs |
| `crates/autopilot/src/startup.rs` | Modified | Added 4 new phases: CheckingPylon, StartingPylon, DetectingCompute, ComputeMixReady |
| `crates/autopilot/src/pylon_integration.rs` | New | Pylon detection, start, backend discovery functions |
| `crates/autopilot/src/lib.rs` | Modified | Export pylon_integration module |
| `crates/autopilot/Cargo.toml` | Modified | Added pylon, reqwest, dirs dependencies |
| `crates/pylon/src/cli/compute.rs` | New | CLI command for compute mix status |
| `crates/pylon/src/cli/mod.rs` | Modified | Added Compute command to enum |
| `crates/pylon/Cargo.toml` | Modified | Added reqwest dependency |
| `src/cli/pylon.rs` | Modified | Added Compute command dispatch |
| `tests/cli_integration.rs` | Modified | Added test_pylon_compute_help test |

## New Startup Phases

The autopilot startup now includes these phases after PreflightComplete:

1. **CheckingPylon** - Checks if pylon daemon is running
2. **StartingPylon** - Auto-starts pylon if not running (with logging)
3. **DetectingCompute** - Detects local backends (Ollama, Apple FM, Llama.cpp, FM-Bridge)
4. **ComputeMixReady** - Displays compute mix summary

## Backends Detected

| Backend | Port | Status | Models |
|---------|------|--------|--------|
| Ollama | 11434 | Running | llama3.2, gpt-oss:120b-cloud, gpt-oss:20b, glm-4.6:cloud, nomic-embed-text |
| Apple FM | 11435 | Not running | - |
| Llama.cpp | 8080 | Not running | - |
| FM-Bridge | 8081 | Not running | - |

## Conclusion

All tests pass. The pylon compute mix integration is working correctly:
- CLI command works with both human-readable and JSON output
- All existing tests continue to pass
- New integration tests for pylon_integration module pass
- Backend detection correctly identifies running services
- Model enumeration works for Ollama
