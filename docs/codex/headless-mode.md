# Headless Mode System

## Overview

The OpenAI Codex CLI provides a robust headless/non-interactive mode system designed for automation, CI/CD pipelines, and unattended execution environments. This system enables programmatic interaction with Codex without requiring user intervention or interactive terminals.

## Core Headless Execution

### Primary Entry Point: `codex exec`

Location: `codex-rs/cli/src/main.rs` (line 55)

The `codex exec` subcommand is specifically designed for non-interactive operation:

```bash
# Basic headless execution
codex exec "analyze this codebase and suggest improvements"

# With JSON output for programmatic consumption
codex exec --json "run tests and report results"

# Reading prompt from stdin
echo "update README.md" | codex exec -

# With output file capture
codex exec --output-last-message response.txt "generate documentation"
```

### Key Features

Location: `codex-rs/exec/src/cli.rs`

- **JSON Output** (`--json`): Emits events as JSONL for programmatic consumption
- **Output File** (`--output-last-message`): Writes agent's final response to a file
- **Stdin Support**: Accepts prompts via stdin using `-` parameter
- **Color Control** (`--color`): Auto/always/never color output control
- **Working Directory** (`--cd`): Configurable working directory

```bash
# Example: CI pipeline usage
codex exec \
  --json \
  --output-last-message build-report.txt \
  --cd /workspace \
  "run the test suite and analyze any failures"
```

## Authentication for Headless Environments

### Environment Variable Authentication

Location: `codex-rs/core/src/auth.rs`

```rust
pub const OPENAI_API_KEY_ENV_VAR: &str = "OPENAI_API_KEY";
```

### Authentication Methods

1. **Environment Variable** (Recommended for CI/CD):
   ```bash
   export OPENAI_API_KEY="your-api-key"
   codex exec "task description"
   ```

2. **Direct Login**:
   ```bash
   codex login --api-key "your-key"
   ```

3. **Auth File Transfer**:
   ```bash
   # Copy authentication between machines
   scp ~/.codex/auth.json remote:~/.codex/
   ```

### Headless Authentication Flows

**SSH Port Forwarding for Remote Login**:
```bash
# Forward login server for OAuth
ssh -L 8080:localhost:8080 remote-server
codex login  # On remote server
```

**Docker Container Setup**:
```dockerfile
# Mount credentials or use env vars
COPY auth.json /root/.codex/auth.json
ENV OPENAI_API_KEY=your-api-key
```

## Approval Policy Management

### Approval Modes

Location: `codex-rs/common/src/approval_mode_cli_arg.rs`

```rust
pub enum ApprovalModeCliArg {
    Untrusted,    // Only trusted commands without approval
    OnFailure,    // Run all commands, ask only on failure
    OnRequest,    // Model decides when to ask (default)
    Never,        // Never ask for approval (full automation)
}
```

### Automation-Friendly Flags

#### `--full-auto` 
Low-friction sandboxed automatic execution:
- Enables workspace-write sandbox mode
- Sets approval policy to on-failure
- Balances safety with automation

#### `--dangerously-bypass-approvals-and-sandbox` (alias: `--yolo`)
Complete bypass for externally sandboxed environments:
- Disables all approval prompts
- Bypasses sandbox restrictions
- Only for trusted/containerized environments

### Example Configurations

```bash
# Safe CI analysis
codex exec --sandbox read-only --ask-for-approval never "analyze code quality"

# Automated development tasks
codex exec --full-auto "run tests and fix any failures"

# Unrestricted (for containers)
codex exec --yolo "perform system maintenance tasks"
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: AI-Powered Code Review
on: [pull_request]

jobs:
  codex-review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Codex CLI
        run: npm install -g @openai/codex
        
      - name: Setup Authentication
        run: codex login --api-key "${{ secrets.OPENAI_API_KEY }}"
        
      - name: AI Code Review
        run: |
          codex exec --json --full-auto \
            "Review the changes in this PR and provide feedback" \
            > review-output.jsonl
            
      - name: Extract Review Comments
        run: |
          # Process JSONL output for PR comments
          cat review-output.jsonl | jq -r '.content // empty'
```

### Session Management for CI

```bash
# Resume previous session
codex exec resume --last

# Resume specific session
codex exec resume "session-id-from-previous-run"

# Append to existing session
codex exec --session existing-session-id "continue previous task"
```

## Configuration for Headless Operation

### Environment Variables

- `OPENAI_API_KEY`: API authentication
- `CODEX_HOME`: Override default state directory (`~/.codex`)
- `RUST_LOG`: Logging configuration

### Configuration File

`~/.codex/config.toml` for automation settings:

```toml
# Automation-friendly configuration
approval_policy = "never"           # No approval prompts
sandbox_mode = "workspace-write"    # Automated file operations
model = "gpt-5"                    # Model selection

[shell_environment_policy]
inherit = "core"                    # Minimal environment inheritance
exclude = ["*_TOKEN", "*_KEY"]      # Security filtering

[tools]
plan_tool = true                    # Enable structured planning
web_search_request = false          # Disable for air-gapped environments
```

### Profile System for Different Automation Contexts

```toml
[profiles.ci_analysis]
approval_policy = "never"
sandbox_mode = "read-only"
model = "gpt-3.5-turbo"

[profiles.ci_development]
approval_policy = "on-failure"
sandbox_mode = "workspace-write"
model = "gpt-5"

[profiles.production_deploy]
approval_policy = "untrusted"
sandbox_mode = "danger-full-access"
model = "gpt-5"
```

Usage:
```bash
codex exec --profile ci_analysis "audit this codebase"
```

## Sandbox Policies for Automation

### Available Modes

1. **Read-Only**: Safe for analysis and reporting
   ```bash
   codex exec --sandbox read-only --ask-for-approval never \
     "analyze test coverage and generate report"
   ```

2. **Workspace-Write**: Limited file operations within workspace
   ```bash
   codex exec --sandbox workspace-write --ask-for-approval on-failure \
     "update documentation based on code changes"
   ```

3. **Danger-Full-Access**: Unrestricted (for containerized environments)
   ```bash
   codex exec --sandbox danger-full-access --ask-for-approval never \
     "perform system updates and restart services"
   ```

### Common CI Combinations

| Use Case | Configuration | Description |
|----------|---------------|-------------|
| **Code Analysis** | `--sandbox read-only --ask-for-approval never` | Safe analysis with no user interaction |
| **Auto Development** | `--full-auto` | Balanced automation with safety checks |
| **Container Deployment** | `--yolo` | Unrestricted for pre-sandboxed environments |

## MCP Server Integration

### Running Codex as MCP Server

```bash
# Start MCP server mode
codex mcp

# Configure for automation
codex mcp --config approval-policy=never --config sandbox=workspace-write
```

### Tool Configuration

```json
{
  "tools": [{
    "name": "codex__exec",
    "properties": {
      "approval-policy": "never",
      "sandbox": "workspace-write", 
      "working-directory": "/workspace"
    }
  }]
}
```

## Error Handling and Logging

### Logging Configuration

**Interactive Mode**:
```bash
RUST_LOG=codex_core=info,codex_tui=info codex
# Logs to ~/.codex/log/codex-tui.log
```

**Headless Mode**:
```bash
RUST_LOG=error codex exec "task"
# Logs inline with output
```

**Debugging**:
```bash
RUST_LOG=debug codex exec --json "task" 2>debug.log
```

### Error Propagation

- **Exit Codes**: Non-zero on failure for shell scripting
- **JSON Events**: Structured error information in JSONL output
- **Timeout Handling**: Configurable timeouts for network operations

### Example Error Handling

```bash
#!/bin/bash
set -e

if codex exec --json "run tests" > test-results.jsonl; then
    echo "Tests passed"
    # Process successful results
    cat test-results.jsonl | jq '.content'
else
    echo "Tests failed with exit code $?"
    # Handle failures
    cat test-results.jsonl | jq '.error // .content'
    exit 1
fi
```

## Interactive vs Headless Comparison

| Feature | Interactive Mode | Headless Mode |
|---------|------------------|---------------|
| **Input Method** | TUI prompts and chat | CLI arguments + stdin |
| **Output Format** | Rich terminal UI | JSONL + file output |
| **User Interaction** | Real-time approval prompts | Pre-configured policies |
| **Error Handling** | Interactive recovery options | Automated retry/fail |
| **Session Management** | Built-in session picker | Explicit session IDs |
| **Logging** | Separate log files | Inline with output |
| **Progress Feedback** | Visual progress indicators | JSON event stream |

## Advanced Automation Features

### Network Configuration

```toml
[network]
timeout_sec = 60
retry_attempts = 3
backoff_multiplier = 2.0

[model_providers.openai]
base_url = "https://api.openai.com/v1"
default_headers = { "User-Agent" = "codex-ci/1.0" }
```

### Tool Selection for Automation

```toml
[tools]
plan_tool = true                          # Structured task management
apply_patch_tool_type = "json"           # Structured file operations
experimental_unified_exec_tool = true    # Advanced command execution
web_search_request = false               # Disable for air-gapped CI
```

### Custom Timeout Configuration

```bash
# Long-running automation tasks
codex exec --timeout 3600 "perform comprehensive analysis"
```

## Security Considerations

### Graduated Security Models

1. **Maximum Security** (Read-only analysis):
   ```bash
   codex exec --sandbox read-only --ask-for-approval never
   ```

2. **Balanced Security** (Controlled automation):
   ```bash
   codex exec --full-auto  # workspace-write + failure escalation
   ```

3. **Minimal Security** (Externally sandboxed):
   ```bash
   codex exec --yolo  # Only for Docker/VM environments
   ```

### Best Practices

1. **Use least privilege**: Start with read-only, escalate as needed
2. **Environment variables**: Prefer `OPENAI_API_KEY` over stored credentials in CI
3. **Session cleanup**: Clean up session data in CI environments
4. **Output capture**: Capture both stdout and stderr for debugging
5. **Timeout protection**: Set appropriate timeouts for automation tasks

The headless mode system provides comprehensive automation capabilities while maintaining security boundaries appropriate for different deployment environments, from secure CI pipelines to containerized execution environments.