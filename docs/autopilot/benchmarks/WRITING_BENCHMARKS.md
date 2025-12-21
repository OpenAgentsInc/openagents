# Writing Benchmark Tasks

This guide explains how to create new benchmark tasks for the autopilot system.

## Benchmark Task Trait

All benchmarks implement the `BenchmarkTask` trait:

```rust
pub trait BenchmarkTask: Send + Sync {
    /// Unique benchmark ID (e.g., "B-001")
    fn id(&self) -> &str;

    /// Human-readable name
    fn name(&self) -> &str;

    /// Task category (file-ops, git, testing, etc.)
    fn category(&self) -> &str;

    /// Set up the benchmark environment
    fn setup(&self, workspace: &Path) -> Result<()>;

    /// Return the prompt to give to the agent
    fn prompt(&self) -> &str;

    /// Validate the result after execution
    fn validate(&self, workspace: &Path) -> Result<ValidationResult>;

    /// Clean up the benchmark environment
    fn teardown(&self, workspace: &Path) -> Result<()>;
}
```

## Step-by-Step Guide

### 1. Choose an ID and Category

Benchmark IDs follow the pattern `B-XXX` where XXX is a zero-padded number:
- B-001, B-002, ... B-999

Categories organize related benchmarks:
- file-ops
- git
- testing
- refactoring
- documentation
- dependencies
- error-handling
- context
- consistency
- performance
- security

### 2. Create the Struct

```rust
/// B-042: Example Task Description
///
/// Task: What the agent needs to accomplish
pub struct B042ExampleTask;
```

### 3. Implement `setup()`

Create the initial environment in the workspace directory:

```rust
fn setup(&self, workspace: &Path) -> Result<()> {
    // Create files
    let config_file = workspace.join("config.yaml");
    std::fs::write(&config_file, "version: 1.0.0\n")?;

    // Initialize git (if needed)
    std::process::Command::new("git")
        .current_dir(workspace)
        .args(["init"])
        .output()?;

    Ok(())
}
```

### 4. Write the Prompt

The prompt should be clear and specific:

```rust
fn prompt(&self) -> &str {
    "Update the version in config.yaml to 1.0.1 and ensure the format is valid YAML"
}
```

### 5. Implement `validate()`

Check if the task was completed correctly:

```rust
fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
    let config_file = workspace.join("config.yaml");
    let content = std::fs::read_to_string(&config_file)?;

    let mut messages = Vec::new();
    let mut custom_metrics = HashMap::new();

    // Check for correct version
    let has_new_version = content.contains("1.0.1");
    if has_new_version {
        messages.push("✓ Version updated to 1.0.1".to_string());
    } else {
        messages.push("✗ Version not updated".to_string());
    }

    // Validate YAML syntax
    let is_valid_yaml = serde_yaml::from_str::<serde_yaml::Value>(&content).is_ok();
    if is_valid_yaml {
        messages.push("✓ Valid YAML syntax".to_string());
    } else {
        messages.push("✗ Invalid YAML syntax".to_string());
    }

    // Record custom metrics
    custom_metrics.insert("file_size".to_string(), content.len() as f64);

    Ok(ValidationResult {
        success: has_new_version && is_valid_yaml,
        messages,
        custom_metrics,
    })
}
```

### 6. Implement `teardown()`

Clean up the workspace:

```rust
fn teardown(&self, workspace: &Path) -> Result<()> {
    if workspace.exists() {
        std::fs::remove_dir_all(workspace)?;
    }
    Ok(())
}
```

## Best Practices

### Make Tasks Realistic

Benchmarks should reflect real development scenarios:
- Use realistic file structures
- Include edge cases
- Test common workflows

### Make Validation Strict

Validation should catch partial or incorrect solutions:
- Check all requirements, not just the main goal
- Verify side effects (file permissions, git state, etc.)
- Test edge cases

### Provide Clear Feedback

Validation messages should be specific:

```rust
// Good
messages.push("✗ Expected 'new_name' in 3 files, found in 1".to_string());

// Bad
messages.push("Failed".to_string());
```

### Use Custom Metrics

Track task-specific measurements:

```rust
custom_metrics.insert("files_modified".to_string(), count as f64);
custom_metrics.insert("lines_changed".to_string(), lines as f64);
custom_metrics.insert("functions_renamed".to_string(), funcs as f64);
```

### Test Incrementally

Verify each step works before combining:

```bash
# Test setup
cargo run --bin test-benchmark-setup B-042

# Test validation logic independently
cargo test benchmark_b042_validation

# Test full benchmark
cargo autopilot benchmark B-042
```

## Common Patterns

### File Creation

```rust
fn setup(&self, workspace: &Path) -> Result<()> {
    let src_dir = workspace.join("src");
    std::fs::create_dir_all(&src_dir)?;

    for i in 1..=5 {
        let file = src_dir.join(format!("module{}.rs", i));
        std::fs::write(&file, format!("// Module {}\n", i))?;
    }

    Ok(())
}
```

### Git Repository Setup

```rust
fn setup(&self, workspace: &Path) -> Result<()> {
    // Initialize git
    std::process::Command::new("git")
        .current_dir(workspace)
        .args(["init"])
        .output()?;

    // Configure git
    std::process::Command::new("git")
        .current_dir(workspace)
        .args(["config", "user.name", "Benchmark"])
        .output()?;

    std::process::Command::new("git")
        .current_dir(workspace)
        .args(["config", "user.email", "bench@example.com"])
        .output()?;

    // Create initial commit
    std::fs::write(workspace.join("README.md"), "# Test\n")?;
    std::process::Command::new("git")
        .current_dir(workspace)
        .args(["add", "."])
        .output()?;
    std::process::Command::new("git")
        .current_dir(workspace)
        .args(["commit", "-m", "Initial commit"])
        .output()?;

    Ok(())
}
```

### Count File Occurrences

```rust
fn validate(&self, workspace: &Path) -> Result<ValidationResult> {
    use walkdir::WalkDir;

    let mut total_count = 0;
    for entry in WalkDir::new(workspace).into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().and_then(|s| s.to_str()) == Some("rs") {
            let content = std::fs::read_to_string(entry.path())?;
            total_count += content.matches("NEW_API").count();
        }
    }

    // ... validation logic
}
```

## Adding to the Runner

After creating your benchmark, register it in the runner:

```rust
// In crates/autopilot/src/benchmark/tasks.rs
pub fn all_benchmarks() -> Vec<Box<dyn BenchmarkTask>> {
    vec![
        Box::new(B001SimpleFileEdit),
        Box::new(B002MultiFileEdit),
        // ... existing benchmarks
        Box::new(B042ExampleTask),  // Add your new benchmark
    ]
}
```

## Example: Complete Benchmark

See `crates/autopilot/src/benchmark/tasks.rs` for complete examples like:
- B-001: Simple file edit (minimal example)
- B-002: Multi-file search and replace
- B-005: Git branch workflow (complex setup)
- B-013: Cross-file consistency (advanced validation)

## Testing Your Benchmark

```bash
# Run just your benchmark
cargo autopilot benchmark B-042

# Run with verbose output
RUST_LOG=debug cargo autopilot benchmark B-042

# Check validation logic
cargo test --package autopilot benchmark_b042
```

## Documentation

Document your benchmark in `docs/autopilot/benchmarks/tasks/B-042.md`:

```markdown
# B-042: Example Task

## Description
Brief description of what this benchmark tests.

## Category
file-ops

## Setup
- Creates config.yaml
- Initializes git repository

## Task
Update version in config.yaml to 1.0.1

## Validation
- Version contains "1.0.1"
- YAML syntax is valid

## Custom Metrics
- file_size: Final size of config.yaml

## Common Failures
- Invalid YAML syntax after edit
- Version not updated completely
```
