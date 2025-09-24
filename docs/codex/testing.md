# Testing System

## Overview

The OpenAI Codex CLI implements a comprehensive testing system spanning from unit tests to end-to-end integration tests. The testing architecture emphasizes quality, reliability, and comprehensive coverage across all components of the CLI application.

## Test Organization and Structure

### Workspace Structure

The codebase is organized as a Rust workspace with 21 crates, each following consistent testing patterns:

```
codex-rs/
├── core/tests/              # Core functionality tests  
├── tui/tests/              # TUI component tests
├── mcp-server/tests/       # MCP server tests
├── apply-patch/tests/      # Patch application tests
├── execpolicy/tests/       # Execution policy tests
└── [crate]/tests/          # Per-crate test suites
```

### Standard Test Pattern

Each crate follows a consistent organization:
- `tests/all.rs` - Single integration test binary entry point
- `tests/suite/` - Individual test modules organized by functionality
- `tests/common/` - Shared test support libraries (where needed)
- `tests/fixtures/` - Test data files and templates

### Test Support Libraries

- **`core_test_support`** - Common testing utilities for core functionality
- **`mcp_test_support`** - MCP (Model Context Protocol) server testing support

## Types of Tests

### Unit Tests
Embedded within source files using `#[cfg(test)]`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_policy_validation() {
        // Test implementation
    }
}
```

### Integration Tests
Comprehensive test suites in `tests/suite/` directories:
- Cross-component interaction testing
- Real CLI command testing using `assert_cmd`
- Full workflow verification

### End-to-End Tests
Complete user journey testing:
- **Login E2E**: `login/tests/suite/login_server_e2e.rs` - OAuth flow testing
- **Command E2E**: `chatgpt/tests/suite/apply_command_e2e.rs` - Full command workflow testing

### UI/Terminal Tests
Sophisticated terminal emulation testing:
- VT100 terminal emulation for accurate TUI testing
- Snapshot-based testing for terminal output verification
- Feature-gated with `vt100-tests` feature

### Snapshot Tests
Extensive use of `insta` crate for regression testing:
- 65+ snapshot files across TUI components
- TUI rendering verification
- Markdown formatting consistency
- Diff display accuracy

## Test Execution

### Primary Test Runner

The project uses `cargo-nextest` for enhanced test execution:

```bash
# Via justfile (preferred)
just test

# Direct execution
cargo nextest run --no-fail-fast

# Fallback to standard cargo
cargo test
```

### Feature-Specific Testing

```bash
# Terminal emulation tests
cargo test --features vt100-tests

# Individual crate testing
cd <crate-directory> && cargo test

# Debug logging
cargo test --features debug-logs
```

### CI Commands

Multi-platform testing with comprehensive coverage:
- Linux (x86_64, ARM64, musl variants)
- macOS (Intel and Apple Silicon)
- Windows (x86_64, ARM)
- Both debug and release profiles

## Test Coverage and Components

### Core Components Tested

**Authentication System**:
- OAuth flows and token management
- API key validation
- Login server integration

**Model Client Integration**:
- OpenAI API communication
- Streaming response handling
- Error condition management

**Conversation Management**:
- Message history persistence
- Session state management
- Context window management

**Shell Execution**:
- Command execution with sandbox policies
- Permission escalation workflows
- Timeout and signal handling

**File Operations**:
- Patch application and file modification
- Diff processing and validation
- File system permission checking

**TUI Components**:
- Terminal rendering and layout
- User input handling
- Interactive command flows

**MCP Server**:
- Protocol implementation
- Message processing
- Tool integration

### Test Categories

1. **Configuration Management**: Loading, validation, and profile handling
2. **Network Communication**: API interactions with comprehensive mocking
3. **File System Operations**: Safe file manipulation and sandbox compliance
4. **Terminal Rendering**: Layout, formatting, and user interaction
5. **Error Handling**: Graceful degradation and recovery mechanisms
6. **Stream Processing**: Real-time data handling and event management

## Test Frameworks and Tools

### Core Testing Infrastructure

```toml
# Key testing dependencies
cargo-nextest = "0.9"         # Primary test runner
assert_cmd = "2.0"            # CLI application testing
wiremock = "0.6"              # HTTP service mocking
insta = "1.39"                # Snapshot testing
pretty_assertions = "1.4"     # Enhanced assertion output
```

### Specialized Testing Tools

- **vt100** - Terminal emulator for TUI testing
- **tokio-test** - Async test utilities
- **tempfile** - Temporary file/directory management
- **ratatui::TestBackend** - TUI component testing
- **predicates** - Flexible assertion predicates

## Fixture Systems and Test Data

### Fixture Organization

```
tests/fixtures/
├── cli_responses_fixture.sse      # Server-sent events templates
├── completed_template.json        # API response templates
├── binary-size-log.jsonl         # Performance tracking data
└── oss-story.jsonl               # Session replay data
```

### Fixture System Features

**SSE Stream Fixtures**: Parameterized templates for server-sent events:
```rust
// Template with placeholder replacement
pub fn load_sse_fixture_with_id(path: impl AsRef<Path>, id: &str) -> String
```

**Session Logs**: JSONL format for TUI interaction replay:
- User input sequences
- Expected output verification
- Performance regression detection

**Template System**: Helper functions for fixture loading and parameterization.

## Mock and Stub Systems

### HTTP Service Mocking

Comprehensive API mocking using `wiremock`:

```rust
// Sequential response testing
let mock_server = MockServer::start().await;
Mock::given(method("POST"))
    .and(path("/v1/chat/completions"))
    .respond_with(ResponseTemplate::new(200))
    .mount(&mock_server)
    .await;
```

### Authentication Mocking

- Mock OAuth issuer implementation
- JWT token generation for testing
- Temporary HTTP servers for integration testing

### Model Provider Mocking

- Configurable response streaming
- Error condition simulation
- Multi-turn conversation testing

### Network-Aware Testing

```rust
// Skip network tests in sandbox environments
non_sandbox_test!(test_network_functionality);
```

## CI/CD Integration

### GitHub Actions Workflows

**Primary Rust CI**: `.github/workflows/rust-ci.yml`
- Multi-platform matrix testing
- Parallel execution for performance
- Comprehensive linting and formatting

### Quality Gates

All CI builds require:
- **Test Success**: All tests must pass with `--no-fail-fast`
- **Zero Warnings**: Strict clippy configuration with `-D warnings`
- **Format Compliance**: Consistent code formatting
- **Feature Validation**: Individual crate feature isolation

### Platform Coverage

**Target Platforms**:
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-musl`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`
- `aarch64-pc-windows-msvc`

## Test Configuration

### Environment Setup

**Workspace Configuration**:
```toml
[workspace]
resolver = "2"
members = [/* 21 crates */]

[workspace.lints.clippy]
# Strict linting rules for quality assurance
```

**Test-Specific Features**:
- `vt100-tests` - Terminal emulation testing
- `debug-logs` - Verbose debugging output
- Environment-based conditional testing

### Test Isolation

- **Temporary Directories**: Each test uses isolated temporary environments
- **Configuration Isolation**: Per-test configuration loading
- **Process Isolation**: Hermetic test execution preventing state pollution

### Development Dependencies

Consistent versioning across workspace:
```toml
[workspace.dev-dependencies]
assert_cmd = "2.0.16"
insta = { version = "1.39.0", features = ["json"] }
pretty_assertions = "1.4.1"
wiremock = "0.6.2"
```

## Running Specific Test Suites

### Individual Components

```bash
# Core functionality
cd codex-rs/core && cargo test

# TUI components with snapshots
cd codex-rs/tui && cargo test --features vt100-tests

# MCP server integration
cd codex-rs/mcp-server && cargo test

# Apply patch functionality
cd codex-rs/apply-patch && cargo test
```

### Test Categories

```bash
# Authentication tests
cargo test auth

# Sandbox policy tests
cargo test sandbox

# Stream processing tests
cargo test stream

# Configuration tests
cargo test config
```

## Test Coverage Analysis

### Covered Areas

1. **High Coverage**:
   - Core business logic (authentication, configuration)
   - UI components (TUI rendering, user interaction)
   - API integration (OpenAI, MCP protocols)
   - File operations (patch application, safe file handling)

2. **Integration Testing**:
   - End-to-end user workflows
   - Cross-component communication
   - External service integration

3. **Edge Cases**:
   - Error condition handling
   - Network failure scenarios
   - Invalid input validation

### Areas for Enhancement

1. **Performance Benchmarks**: No dedicated benchmark testing infrastructure
2. **Property-Based Testing**: No evidence of property-based or fuzz testing
3. **Coverage Metrics**: No visible coverage measurement tooling
4. **Load Testing**: No stress testing for concurrent operations

## Key Testing Principles

1. **Hermetic Testing**: Each test runs in isolation with its own environment
2. **Comprehensive Mocking**: External dependencies are consistently mocked
3. **Snapshot Consistency**: UI and formatting regression prevention
4. **Multi-Platform Validation**: Ensuring compatibility across all target platforms
5. **CI Integration**: Automated testing preventing regression
6. **Performance Focus**: Fast test execution using cargo-nextest

The testing system demonstrates enterprise-grade quality assurance with comprehensive coverage from low-level policy enforcement to high-level user interface interactions, ensuring reliability and maintainability of the OpenAI Codex CLI.
