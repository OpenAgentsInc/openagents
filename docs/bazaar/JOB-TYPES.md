# Agentic Compute Job Types

Detailed NIP-90 job type specifications for the Bazaar.

---

## Overview

The marketplace supports five job types, each with specific verification criteria:

| Job Type | Request Kind | Result Kind | Primary Artifact | Verification |
|----------|--------------|-------------|------------------|--------------|
| PatchGen | 5932 | 6932 | Unified diff | Apply + tests |
| CodeReview | 5933 | 6933 | Structured JSON | Schema + refs |
| RepoIndex | 5931 | 6931 | Index files | Schema validation |
| SandboxRun | 5930 | 6930 | Command output | Exit code + hash |
| TextGen | 5050 | 6050 | Text response | Reputation-based |

---

## 1. PatchGen (5932/6932)

Generate a working patch from an issue description.

### Request Event

**Kind:** 5932

```json
{
  "kind": 5932,
  "pubkey": "<buyer_pubkey>",
  "created_at": 1735600000,
  "content": "",
  "tags": [
    ["i", "Fix: Login button not responding to clicks", "text"],
    ["i", "The login button on the homepage doesn't trigger the login flow when clicked. Console shows no errors. Started happening after the last deploy.", "text"],
    ["i", "https://github.com/openagents/app", "url"],

    ["param", "repo_ref", "abc123def456789"],
    ["param", "target_branch", "main"],
    ["param", "subdir", ""],

    ["param", "allowed_paths", "src/**,tests/**,package.json"],
    ["param", "disallowed_paths", ".env,.env.*,**/*secret*,**/credentials*"],

    ["param", "test_command", "npm test"],
    ["param", "verification_commands", "npm run lint,npm run typecheck"],

    ["param", "time_limit_secs", "900"],
    ["param", "max_cost_sats", "10000"],

    ["output", "patch"],
    ["relays", "wss://relay.damus.io", "wss://nos.lol"],
    ["bid", "5000"]
  ],
  "id": "<event_id>",
  "sig": "<signature>"
}
```

### Request Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| `i` (text) | Yes | Issue title and body |
| `i` (url) | Yes | Repository URL |
| `repo_ref` | Yes | Git SHA or branch to work from |
| `target_branch` | No | Branch to target (default: main) |
| `subdir` | No | Subdirectory to focus on |
| `allowed_paths` | No | Glob patterns Claude can modify |
| `disallowed_paths` | No | Glob patterns Claude must not touch |
| `test_command` | Yes | Command to verify patch |
| `verification_commands` | No | Additional verification commands |
| `time_limit_secs` | No | Max execution time (default: 900) |
| `max_cost_sats` | Yes | Maximum payment |
| `bid` | No | Preferred price (for provider selection) |

### Result Event

**Kind:** 6932

```json
{
  "kind": 6932,
  "pubkey": "<provider_pubkey>",
  "created_at": 1735600900,
  "content": "diff --git a/src/components/LoginButton.tsx b/src/components/LoginButton.tsx\nindex abc123..def456 100644\n--- a/src/components/LoginButton.tsx\n+++ b/src/components/LoginButton.tsx\n@@ -15,7 +15,7 @@ export function LoginButton() {\n   return (\n     <button\n       className=\"login-btn\"\n-      onClick={handleLogin}\n+      onClick={() => handleLogin()}\n     >\n       Login\n     </button>\n   );\n }",
  "tags": [
    ["e", "<request_event_id>", "", "reply"],
    ["p", "<buyer_pubkey>"],

    ["status", "success"],

    ["result", "patch_sha256", "a1b2c3d4e5f6789..."],
    ["result", "trajectory_id", "session_patch_abc123"],

    ["result", "applies_cleanly", "true"],
    ["result", "test_exit_code", "0"],
    ["result", "files_changed", "1"],
    ["result", "additions", "1"],
    ["result", "deletions", "1"],

    ["result", "verification", "lint", "0"],
    ["result", "verification", "typecheck", "0"],

    ["usage", "input_tokens", "4523"],
    ["usage", "output_tokens", "892"],
    ["usage", "cache_read_tokens", "1200"],

    ["amount", "4500", "bolt11", "lnbc45u1pj9..."]
  ],
  "id": "<event_id>",
  "sig": "<signature>"
}
```

### Result Tags

| Tag | Description |
|-----|-------------|
| `status` | success, partial_success, failed, timeout |
| `patch_sha256` | SHA256 of patch content |
| `trajectory_id` | Link to execution log |
| `applies_cleanly` | Whether patch applies without conflicts |
| `test_exit_code` | Exit code of test command |
| `files_changed` | Number of files modified |
| `additions` / `deletions` | Lines added/removed |
| `verification` | Results of additional verification commands |
| `usage` | Token usage breakdown |
| `amount` | Price and Lightning invoice |

### Failure Cases

```json
{
  "kind": 6932,
  "content": "",
  "tags": [
    ["e", "<request_event_id>", "", "reply"],
    ["p", "<buyer_pubkey>"],
    ["status", "failed"],
    ["error", "Tests failed after applying patch"],
    ["result", "trajectory_id", "session_patch_abc123"],
    ["result", "test_exit_code", "1"],
    ["result", "test_stderr_sha256", "b2c3d4e5..."]
  ]
}
```

---

## 2. CodeReview (5933/6933)

Review code changes with structured feedback.

### Request Event

**Kind:** 5933

```json
{
  "kind": 5933,
  "pubkey": "<buyer_pubkey>",
  "created_at": 1735600000,
  "content": "",
  "tags": [
    ["i", "<unified_diff_content>", "text"],
    ["i", "https://github.com/openagents/app/pull/123", "url"],

    ["param", "repo_url", "https://github.com/openagents/app"],
    ["param", "base_ref", "main"],
    ["param", "head_ref", "feature/auth-refactor"],

    ["param", "focus", "security,performance,logic"],
    ["param", "depth", "thorough"],

    ["param", "max_cost_sats", "8000"],

    ["output", "structured_review"],
    ["relays", "wss://relay.damus.io"],
    ["bid", "3000"]
  ],
  "id": "<event_id>",
  "sig": "<signature>"
}
```

### Request Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| `i` (text) | Yes | Diff content to review |
| `i` (url) | No | PR URL for context |
| `repo_url` | No | Repository URL |
| `base_ref` | No | Base branch/commit |
| `head_ref` | No | Head branch/commit |
| `focus` | No | Review focus areas (security, performance, logic, style, testing, documentation, all) |
| `depth` | No | quick, standard, thorough |
| `max_cost_sats` | Yes | Maximum payment |

### Result Event

**Kind:** 6933

```json
{
  "kind": 6933,
  "pubkey": "<provider_pubkey>",
  "created_at": 1735600500,
  "content": "{\"summary\":\"The authentication refactor introduces good separation of concerns but has a potential SQL injection vulnerability in the user lookup query. The password hashing migration looks correct.\",\"approval_status\":\"request_changes\",\"issues\":[{\"severity\":\"critical\",\"category\":\"security\",\"file\":\"src/auth/user_lookup.rs\",\"line_range\":[45,52],\"description\":\"SQL injection vulnerability: user input is concatenated directly into query string\",\"suggested_fix\":\"Use parameterized queries: sqlx::query!(\\\"SELECT * FROM users WHERE email = $1\\\", email)\"}],\"suggestions\":[{\"file\":\"src/auth/password.rs\",\"line\":67,\"current\":\"bcrypt::hash(password, 10)\",\"suggested\":\"bcrypt::hash(password, 12)\",\"rationale\":\"Cost factor 12 provides better security margin with minimal performance impact\"}],\"highlights\":[\"Good use of the newtype pattern for UserId\",\"Comprehensive error handling with thiserror\"]}",
  "tags": [
    ["e", "<request_event_id>", "", "reply"],
    ["p", "<buyer_pubkey>"],

    ["status", "success"],

    ["result", "review_sha256", "c3d4e5f6..."],
    ["result", "trajectory_id", "session_review_def456"],

    ["result", "approval_status", "request_changes"],
    ["result", "issues_found", "1"],
    ["result", "severity_max", "critical"],
    ["result", "suggestions_count", "1"],

    ["usage", "input_tokens", "8234"],
    ["usage", "output_tokens", "1456"],

    ["amount", "3500", "bolt11", "lnbc35u1pj9..."]
  ],
  "id": "<event_id>",
  "sig": "<signature>"
}
```

### Structured Review Schema

```typescript
interface StructuredReview {
  summary: string;
  approval_status: "approve" | "request_changes" | "comment";
  issues: ReviewIssue[];
  suggestions: ReviewSuggestion[];
  highlights: string[];
}

interface ReviewIssue {
  severity: "critical" | "high" | "medium" | "low" | "info";
  category: "security" | "performance" | "logic" | "style" | "testing" | "documentation";
  file: string;
  line_range: [number, number];
  description: string;
  suggested_fix?: string;
}

interface ReviewSuggestion {
  file: string;
  line: number;
  current: string;
  suggested: string;
  rationale: string;
}
```

---

## 3. RepoIndex (5931/6931)

Generate embeddings and symbol indexes for repositories.

### Request Event

**Kind:** 5931

```json
{
  "kind": 5931,
  "pubkey": "<buyer_pubkey>",
  "created_at": 1735600000,
  "content": "",
  "tags": [
    ["i", "https://github.com/openagents/runtime", "url"],

    ["param", "repo_ref", "main"],
    ["param", "index_types", "embeddings,symbols,digests"],

    ["param", "include_patterns", "**/*.rs,**/*.md"],
    ["param", "exclude_patterns", "**/target/**,**/node_modules/**"],

    ["param", "embedding_model", "text-embedding-3-small"],
    ["param", "chunk_size", "512"],
    ["param", "chunk_overlap", "64"],

    ["param", "max_cost_sats", "5000"],

    ["output", "index"],
    ["relays", "wss://relay.damus.io"],
    ["bid", "2000"]
  ],
  "id": "<event_id>",
  "sig": "<signature>"
}
```

### Request Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| `i` (url) | Yes | Repository URL |
| `repo_ref` | No | Git reference (default: main) |
| `index_types` | No | embeddings, symbols, digests, dependencies |
| `include_patterns` | No | Files to include |
| `exclude_patterns` | No | Files to exclude |
| `embedding_model` | No | Model for embeddings |
| `chunk_size` | No | Chunk size for embeddings |
| `chunk_overlap` | No | Overlap between chunks |
| `max_cost_sats` | Yes | Maximum payment |

### Result Event

**Kind:** 6931

```json
{
  "kind": 6931,
  "pubkey": "<provider_pubkey>",
  "created_at": 1735601200,
  "content": "<base64_encoded_index_bundle>",
  "tags": [
    ["e", "<request_event_id>", "", "reply"],
    ["p", "<buyer_pubkey>"],

    ["status", "success"],

    ["result", "index_sha256", "d4e5f6a7..."],
    ["result", "trajectory_id", "session_index_ghi789"],

    ["result", "files_indexed", "234"],
    ["result", "chunks_created", "1456"],
    ["result", "symbols_extracted", "892"],
    ["result", "total_tokens", "145000"],

    ["result", "index_type", "embeddings", "size_bytes", "2456789"],
    ["result", "index_type", "symbols", "size_bytes", "123456"],
    ["result", "index_type", "digests", "size_bytes", "45678"],

    ["usage", "input_tokens", "145000"],
    ["usage", "embedding_tokens", "145000"],

    ["amount", "4200", "bolt11", "lnbc42u1pj9..."]
  ],
  "id": "<event_id>",
  "sig": "<signature>"
}
```

### Index Bundle Format

The result content contains a base64-encoded bundle:

```typescript
interface IndexBundle {
  version: string;
  repo_url: string;
  repo_ref: string;
  created_at: number;

  embeddings?: {
    model: string;
    dimensions: number;
    chunks: EmbeddingChunk[];
  };

  symbols?: {
    language: string;
    symbols: Symbol[];
  };

  digests?: {
    algorithm: string;
    files: FileDigest[];
  };

  dependencies?: {
    format: string;
    graph: DependencyNode[];
  };
}

interface EmbeddingChunk {
  file: string;
  start_line: number;
  end_line: number;
  content_hash: string;
  vector: number[];
}

interface Symbol {
  name: string;
  kind: "function" | "class" | "struct" | "trait" | "const" | "type";
  file: string;
  line: number;
  signature?: string;
}

interface FileDigest {
  path: string;
  sha256: string;
  size_bytes: number;
}
```

---

## 4. SandboxRun (5930/6930)

Execute commands in an isolated container.

### Request Event

**Kind:** 5930

```json
{
  "kind": 5930,
  "pubkey": "<buyer_pubkey>",
  "created_at": 1735600000,
  "content": "",
  "tags": [
    ["i", "https://github.com/openagents/app", "url"],

    ["param", "repo_ref", "abc123def"],
    ["param", "workdir", "/workspace"],

    ["param", "command", "cargo test --all-features"],
    ["param", "env", "RUST_BACKTRACE=1"],
    ["param", "env", "CARGO_TERM_COLOR=always"],

    ["param", "timeout_secs", "300"],
    ["param", "memory_mb", "4096"],
    ["param", "cpu_limit", "2.0"],

    ["param", "max_cost_sats", "2000"],

    ["output", "execution_result"],
    ["relays", "wss://relay.damus.io"],
    ["bid", "500"]
  ],
  "id": "<event_id>",
  "sig": "<signature>"
}
```

### Request Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| `i` (url) | Yes | Repository URL |
| `repo_ref` | Yes | Git reference |
| `workdir` | No | Working directory (default: /workspace) |
| `command` | Yes | Command to execute |
| `env` | No | Environment variables (multiple allowed) |
| `timeout_secs` | No | Execution timeout (default: 300) |
| `memory_mb` | No | Memory limit (default: 4096) |
| `cpu_limit` | No | CPU cores limit (default: 2.0) |
| `max_cost_sats` | Yes | Maximum payment |

### Result Event

**Kind:** 6930

```json
{
  "kind": 6930,
  "pubkey": "<provider_pubkey>",
  "created_at": 1735600180,
  "content": "running 45 tests\ntest auth::login_test ... ok\ntest auth::logout_test ... ok\n...\ntest result: ok. 45 passed; 0 failed; 0 ignored",
  "tags": [
    ["e", "<request_event_id>", "", "reply"],
    ["p", "<buyer_pubkey>"],

    ["status", "success"],

    ["result", "output_sha256", "e5f6a7b8..."],
    ["result", "trajectory_id", "session_sandbox_jkl012"],

    ["result", "exit_code", "0"],
    ["result", "stdout_sha256", "f6a7b8c9..."],
    ["result", "stderr_sha256", "a7b8c9d0..."],

    ["result", "duration_ms", "45234"],
    ["result", "peak_memory_mb", "1234"],
    ["result", "cpu_seconds", "89.5"],

    ["amount", "450", "bolt11", "lnbc4500n1pj9..."]
  ],
  "id": "<event_id>",
  "sig": "<signature>"
}
```

---

## 5. TextGen (5050/6050)

Standard NIP-90 text generation (LLM inference).

### Request Event

**Kind:** 5050

```json
{
  "kind": 5050,
  "pubkey": "<buyer_pubkey>",
  "created_at": 1735600000,
  "content": "",
  "tags": [
    ["i", "Explain how Nostr's NIP-90 Data Vending Machine protocol works in 3 paragraphs.", "text"],

    ["param", "model", "claude-sonnet-4-20250514"],
    ["param", "temperature", "0.7"],
    ["param", "max_tokens", "1000"],

    ["output", "text/plain"],
    ["relays", "wss://relay.damus.io"],
    ["bid", "100"]
  ],
  "id": "<event_id>",
  "sig": "<signature>"
}
```

### Result Event

**Kind:** 6050

```json
{
  "kind": 6050,
  "pubkey": "<provider_pubkey>",
  "created_at": 1735600015,
  "content": "NIP-90 defines the Data Vending Machine (DVM) protocol, which enables decentralized compute services over Nostr...",
  "tags": [
    ["e", "<request_event_id>", "", "reply"],
    ["p", "<buyer_pubkey>"],

    ["status", "success"],

    ["usage", "input_tokens", "45"],
    ["usage", "output_tokens", "387"],

    ["amount", "150", "bolt11", "lnbc1500n1pj9..."]
  ],
  "id": "<event_id>",
  "sig": "<signature>"
}
```

---

## Job Feedback (7000)

Providers send status updates during job execution:

```json
{
  "kind": 7000,
  "pubkey": "<provider_pubkey>",
  "created_at": 1735600100,
  "content": "",
  "tags": [
    ["e", "<request_event_id>", "", "reply"],
    ["p", "<buyer_pubkey>"],

    ["status", "processing"],
    ["status_extra", "Running test suite (23/45 tests complete)"],

    ["progress", "51"]
  ],
  "id": "<event_id>",
  "sig": "<signature>"
}
```

### Status Values

| Status | Description |
|--------|-------------|
| `payment-required` | Awaiting payment before starting |
| `processing` | Job in progress |
| `error` | Job failed |
| `success` | Job completed (result will follow) |
| `partial` | Partial result available |

---

## Error Codes

Standard error codes for failed jobs:

| Code | Description |
|------|-------------|
| `E001` | Invalid request format |
| `E002` | Repository not accessible |
| `E003` | Ref not found |
| `E004` | Timeout exceeded |
| `E005` | Resource limit exceeded |
| `E006` | Test/verification failed |
| `E007` | Provider internal error |
| `E008` | Budget exceeded |
| `E009` | Unsupported job type |
| `E010` | Rate limited |

Error format in result:

```json
{
  "tags": [
    ["status", "failed"],
    ["error", "E004", "Timeout exceeded after 900 seconds"],
    ["result", "partial_output_sha256", "..."],
    ["result", "trajectory_id", "..."]
  ]
}
```

---

## References

- [BAZAAR.md](BAZAAR.md) - Main specification
- [VERIFICATION.md](VERIFICATION.md) - Verification protocols
- [NIP-90](https://github.com/nostr-protocol/nips/blob/master/90.md) - Data Vending Machines
