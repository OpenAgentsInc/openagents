# The Bazaar

An open market for agent work. Bring your agent. Sell results.

---

## The Cathedral and the Bazaar

Eric S. Raymond contrasted two ways software gets built:

- **Cathedral**: Centrally planned, closed, release rarely, one "priesthood" decides what ships
- **Bazaar**: Open participation, many parallel contributors, fast iteration, ideas compete in public, value emerges from the crowd

The Bazaar is literally a bazaar: anyone can show up with capability (their Claude, their agent, their compute), take work, get paid, build reputation, and iterate.

**What the Bazaar promises:**
- **Open entry**: Anyone can supply work (agents, skills, compute)
- **Price discovery**: Market-based pricing, not opaque SaaS tiers
- **Composability**: Jobs, skills, and providers mix and match
- **Proof / Provenance**: Receipts, logs, reputation—because buyers can't "trust the mall"
- **Fluid routing**: The system chooses the best stall for the job

**Not reselling models—clearing work.**

---

## Executive Summary

The Bazaar enables a two-sided economy:

- **Providers** contribute agent compute capacity (their own coding agents)
- **Buyers** (primarily Autopilot) purchase verifiable work products
- **Settlement** happens via Lightning with pay-after-verify semantics

**Core Principle:** Agent monetization == selling verifiable contracts.

We do NOT sell "Claude access" or allow arbitrary prompts from strangers. We sell **work products with objective verification** - patches that apply and pass tests, reviews that reference real code, indexes that validate against schema.

### The Flywheel

```
Users pay Autopilot
  → Autopilot buys verifiable jobs from the mesh
    → Contributors earn Bitcoin using their own coding agents
      → More contributors come online (more supply, lower prices)
        → Autopilot gets cheaper/faster (better ROI)
          → More users pay Autopilot
```

### Why This Works (The Bazaar Advantage)

1. **Many small bets**: Try a stall. If it's good, come back. Pay-per-job means low trust requirements at the start, and reputation compounds into higher-priced work.

2. **Competition at the unit-of-work level**: Bazaar vendors compete *per job* ("I can do that task faster/cheaper/better"), not at the platform level.

3. **Visibility → trust → liquidity**: The HUD and trajectories are a modern analogue of "open source visibility"—transparent work logs, signed receipts, public reputation.

4. **"Release early, release often" → "clear often"**: Many small jobs, verified outputs, fast settlement, tight feedback loops.

5. **No platform monopoly**: No single vendor owns supply, no single lab owns distribution. Bring your own Claude, bring your own agent, bring your own hardware.

### What This Document Covers

1. **Provider Architecture** - How contributors run provider nodes
2. **Job Types** - PatchGen, CodeReview, and other verifiable contracts
3. **Verification System** - How buyers verify work before paying
4. **Economic Model** - Pricing, payments, revenue splits
5. **Routing & Discovery** - How buyers find and select providers
6. **Integration Points** - How this connects to Autopilot, runtime, and Neobank

---

## 1. Provider Architecture

### 1.1 Overview

Contributors run provider nodes using the `pylon` infrastructure, extended with Claude worker capabilities:

```
┌─────────────────────────────────────────────────────────────────────┐
│                        PROVIDER NODE (Pylon)                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  │
│  │  Claude Worker   │  │  Claude Worker   │  │  LLM Backend     │  │
│  │  (Agent SDK)     │  │  (Agent SDK)     │  │  (Ollama/etc)    │  │
│  └────────┬─────────┘  └────────┬─────────┘  └────────┬─────────┘  │
│           └────────────────┬────────────────────────────┘           │
│                            │                                        │
│  ┌─────────────────────────┴─────────────────────────────────────┐  │
│  │                      Worker Pool Manager                       │  │
│  │  • Job routing by type (PatchGen, CodeReview, Inference)      │  │
│  │  • Capacity tracking and health monitoring                     │  │
│  │  • Isolation enforcement (container/gvisor/firecracker)        │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│                            │                                        │
│  ┌─────────────────────────┴─────────────────────────────────────┐  │
│  │                        DVM Service                             │  │
│  │  • NIP-90 job request handling                                │  │
│  │  • NIP-89 capability announcements                            │  │
│  │  • Nostr relay connections                                    │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│                            │                                        │
│  ┌──────────────┐  ┌───────┴──────────┐  ┌──────────────────────┐  │
│  │   Neobank    │  │  Unified Identity │  │  Trajectory Logger  │  │
│  │   Treasury   │  │  (BIP39/FROSTR)   │  │  (Receipts)         │  │
│  └──────────────┘  └──────────────────┘  └──────────────────────┘  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 Security Model: Credentials Stay Local

**Critical requirement:** Provider Claude credentials NEVER leave the provider's machine.

The security model uses tunnel-based authentication (from `/claude` mount architecture):

```
Provider Machine                     Mesh / Buyers
├── Anthropic API Key                     │
├── Local Claude Proxy                    │
│   ├── Handles authentication            │
│   ├── Routes to Claude (Anthropic API)  │
│   └── Tracks usage/limits               │
└── Tunnel Endpoint ◄────────────────────────── Job Requests
    ├── wss://provider.ngrok.io/claude         (via Nostr)
    ├── Nostr-signed authentication
    └── Domain allowlist enforcement
```

**Key security properties:**
- API keys stay on provider's machine
- Provider controls rate limits and model selection
- Buyers authenticate via Nostr signatures
- Network isolation via `--network none` containers with proxy socket

### 1.3 Worker Isolation Modes

| Mode | Isolation | Network | Use Case |
|------|-----------|---------|----------|
| `local` | Process | Host network | Dev, trusted agents |
| `container` | Docker | None (proxy socket) | Production, untrusted |
| `gvisor` | gVisor sandbox | None (proxy socket) | High security |
| `firecracker` | microVM | None (proxy socket) | Maximum isolation |

For untrusted workloads (marketplace jobs), the container configuration:

```bash
docker run \
  --network none \
  --cap-drop ALL \
  --read-only \
  --tmpfs /workspace:rw,noexec,nosuid \
  --pids-limit 100 \
  --memory 4g \
  -v /var/run/anthropic-proxy.sock:/var/run/anthropic-proxy.sock:ro \
  -v /filtered-repo:/repo:ro \
  claude-worker
```

### 1.4 Provider CLI

```bash
# Start provider with Claude compute
openagents provider serve --claude-code \
  --capacity 3 \                    # Max concurrent Claude sessions
  --job-types "PatchGen,CodeReview" \
  --isolation container \           # container|gvisor|firecracker
  --tunnel ngrok                    # Tunnel provider for Claude proxy

# Check provider status
openagents provider status

# View earnings
openagents provider earnings --period week
```

### 1.5 Core Types

```rust
/// Agent job types supported by the marketplace
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentJobType {
    /// Generate patch from issue (NIP-90 kind 5932)
    PatchGen,
    /// Code review with suggested fixes (NIP-90 kind 5933)
    CodeReview,
    /// Repository analysis/indexing (NIP-90 kind 5931)
    RepoIndex,
    /// Generic sandbox execution (NIP-90 kind 5930)
    SandboxRun,
    /// Simple inference (NIP-90 kind 5050)
    TextGen,
}

impl AgentJobType {
    pub fn request_kind(&self) -> u16 {
        match self {
            Self::PatchGen => 5932,
            Self::CodeReview => 5933,
            Self::RepoIndex => 5931,
            Self::SandboxRun => 5930,
            Self::TextGen => 5050,
        }
    }

    pub fn result_kind(&self) -> u16 {
        self.request_kind() + 1000
    }
}

/// Claude worker pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeWorkerPool {
    /// Number of concurrent workers
    pub capacity: u32,
    /// Isolation mode per worker
    pub isolation: IsolationMode,
    /// Supported job types
    pub job_types: Vec<AgentJobType>,
    /// Per-type pricing (sats)
    pub pricing: HashMap<AgentJobType, JobPricing>,
    /// Tool allowlist for Claude sessions
    pub allowed_tools: Vec<String>,
    /// Model preference (e.g., "claude-sonnet-4-*")
    pub model_pattern: String,
    /// Maximum context tokens per session
    pub max_context_tokens: u64,
    /// Tunnel configuration
    pub tunnel: TunnelConfig,
}

/// Worker isolation mode
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationMode {
    /// Process on host (dev only)
    Local,
    /// Docker container with hardening
    #[default]
    Container,
    /// gVisor sandbox (stronger)
    Gvisor,
    /// Firecracker microVM (strongest)
    Firecracker,
}

/// Pricing for a job type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobPricing {
    /// Base price in sats
    pub base_sats: u64,
    /// Per-unit price (interpretation depends on job type)
    pub per_unit_sats: u64,
    /// Unit type for per_unit_sats
    pub unit_type: PricingUnit,
    /// Maximum price cap
    pub max_sats: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PricingUnit {
    /// Per file changed (PatchGen, CodeReview)
    PerFile,
    /// Per 1k tokens (TextGen, RepoIndex)
    Per1kTokens,
    /// Per CPU-second (SandboxRun)
    PerCpuSecond,
    /// Flat rate (no variable component)
    Flat,
}

/// Tunnel configuration for Claude proxy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    /// Tunnel provider (ngrok, cloudflare, nostr)
    pub provider: TunnelProvider,
    /// Tunnel endpoint URL (auto-configured or manual)
    pub endpoint_url: Option<String>,
    /// Authentication method
    pub auth: TunnelAuth,
    /// Domain allowlist for egress
    pub proxy_allowlist: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelProvider {
    Ngrok,
    Cloudflare,
    Nostr,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TunnelAuth {
    /// No authentication (dangerous, local only)
    None,
    /// Nostr signature authentication
    Nostr { relay: Option<String> },
    /// Pre-shared key
    Psk { secret_path: String },
}
```

---

## 2. Job Types

### 2.1 PatchGen (Kind 5932/6932)

**Purpose:** Generate a working patch from an issue description or bug report.

This is the flagship job type - the most compelling demonstration of "Claude doing real work."

#### Request Schema

```rust
/// PatchGen job request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchGenRequest {
    /// Issue title
    pub title: String,
    /// Issue body (markdown)
    pub body: String,
    /// Repository URL
    pub repo_url: String,
    /// Git reference (SHA or branch)
    pub repo_ref: String,
    /// Optional subdirectory to focus on
    pub subdir: Option<String>,
    /// Paths Claude is allowed to modify
    pub allowed_paths: Vec<String>,
    /// Paths Claude must not touch
    pub disallowed_paths: Vec<String>,
    /// Test command to run for verification
    pub test_command: String,
    /// Additional verification commands
    pub verification_commands: Vec<String>,
    /// Maximum cost in sats
    pub max_cost_sats: u64,
    /// Time limit in seconds
    pub time_limit_secs: u64,
    /// Idempotency key
    pub idempotency_key: String,
}
```

#### NIP-90 Event (Kind 5932)

```json
{
  "kind": 5932,
  "content": "Generate a patch for the following issue",
  "tags": [
    ["i", "<issue_title>: <issue_body>", "text"],
    ["i", "https://github.com/user/repo", "url"],
    ["param", "repo_ref", "abc123def456"],
    ["param", "allowed_paths", "src/**,tests/**"],
    ["param", "disallowed_paths", ".env,**/*secret*"],
    ["param", "test_command", "cargo test"],
    ["param", "time_limit_secs", "900"],
    ["param", "max_cost_sats", "10000"],
    ["output", "patch"],
    ["bid", "5000"]
  ]
}
```

#### Result Schema

```rust
/// PatchGen job result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchGenResult {
    /// Status of the job
    pub status: JobResultStatus,
    /// The generated patch (unified diff format)
    pub patch: Option<String>,
    /// SHA256 hash of the patch
    pub patch_sha256: String,
    /// Trajectory session ID (for auditing)
    pub trajectory_id: String,
    /// Test verification results
    pub verification: PatchVerification,
    /// Token usage
    pub usage: TokenUsage,
    /// Actual cost in sats
    pub cost_sats: u64,
    /// Lightning invoice for payment
    pub invoice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobResultStatus {
    Success,
    PartialSuccess,
    Failed { error: String },
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchVerification {
    /// Patch applies cleanly
    pub applies_cleanly: bool,
    /// Test command exit code
    pub test_exit_code: i32,
    /// Number of files changed
    pub files_changed: u32,
    /// Lines added
    pub additions: u32,
    /// Lines deleted
    pub deletions: u32,
    /// Verification command results
    pub verification_results: Vec<CommandResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub command: String,
    pub exit_code: i32,
    pub stdout_sha256: String,
    pub stderr_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}
```

#### NIP-90 Result Event (Kind 6932)

```json
{
  "kind": 6932,
  "content": "<unified_diff_patch>",
  "tags": [
    ["e", "<request_event_id>"],
    ["p", "<requester_pubkey>"],
    ["status", "success"],
    ["result", "patch_sha256", "a1b2c3d4..."],
    ["result", "trajectory_id", "session_abc123"],
    ["result", "test_exit_code", "0"],
    ["result", "files_changed", "3"],
    ["result", "additions", "47"],
    ["result", "deletions", "12"],
    ["amount", "5000", "bolt11", "lnbc50u1p..."]
  ]
}
```

### 2.2 CodeReview (Kind 5933/6933)

**Purpose:** Review code changes with structured feedback and suggested fixes.

#### Request Schema

```rust
/// CodeReview job request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeReviewRequest {
    /// PR or diff to review
    pub diff: String,
    /// Repository URL for context
    pub repo_url: String,
    /// Git reference for base
    pub base_ref: String,
    /// Git reference for head
    pub head_ref: String,
    /// Review focus areas
    pub focus: Vec<ReviewFocus>,
    /// Review depth
    pub depth: ReviewDepth,
    /// Maximum cost in sats
    pub max_cost_sats: u64,
    /// Idempotency key
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFocus {
    Security,
    Performance,
    Logic,
    Style,
    Testing,
    Documentation,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDepth {
    /// Quick scan for obvious issues
    Quick,
    /// Standard review
    Standard,
    /// Deep analysis with alternative implementations
    Thorough,
}
```

#### Result Schema

```rust
/// CodeReview job result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeReviewResult {
    /// Status of the job
    pub status: JobResultStatus,
    /// Structured review content
    pub review: StructuredReview,
    /// SHA256 hash of the review
    pub review_sha256: String,
    /// Trajectory session ID
    pub trajectory_id: String,
    /// Token usage
    pub usage: TokenUsage,
    /// Actual cost in sats
    pub cost_sats: u64,
    /// Lightning invoice
    pub invoice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredReview {
    /// Overall summary
    pub summary: String,
    /// Approval status
    pub approval_status: ApprovalStatus,
    /// Issues found
    pub issues: Vec<ReviewIssue>,
    /// Improvement suggestions
    pub suggestions: Vec<ReviewSuggestion>,
    /// Positive highlights
    pub highlights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Approve,
    RequestChanges,
    Comment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewIssue {
    /// Severity level
    pub severity: Severity,
    /// Category
    pub category: ReviewFocus,
    /// File path
    pub file: String,
    /// Line range (start, end)
    pub line_range: (u32, u32),
    /// Issue description
    pub description: String,
    /// Suggested fix (optional)
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSuggestion {
    /// File path
    pub file: String,
    /// Line number
    pub line: u32,
    /// Current code
    pub current: String,
    /// Suggested code
    pub suggested: String,
    /// Rationale
    pub rationale: String,
}
```

### 2.3 Existing Job Types

These are already implemented in the codebase:

| Job Type | Kind | Location |
|----------|------|----------|
| SandboxRun | 5930/6930 | `crates/compute/src/domain/sandbox_run.rs` |
| RepoIndex | 5931/6931 | `crates/compute/src/domain/repo_index.rs` |
| TextGen | 5050/6050 | `crates/nostr/core/src/nip90.rs` |

### 2.4 What Makes These "Verifiable Contracts"

Each job type produces artifacts that enable pay-after-verify:

| Job Type | Primary Artifact | Verification Method |
|----------|-----------------|---------------------|
| **PatchGen** | Unified diff | Apply patch, run tests, check exit code |
| **CodeReview** | Structured JSON | Schema validation, reference checking |
| **SandboxRun** | Command output | Exit code, output hash |
| **RepoIndex** | Index files | Schema validation, spot-check queries |
| **TextGen** | Text response | Reputation-based (less verifiable) |

**Universal verification artifact:** Every job produces a trajectory ID linking to a full session log that records all tool calls, thinking blocks, and timing.

---

## 3. Verification System

### 3.1 Verification Flow

```
                                  ┌──────────────────┐
                                  │   AUTOPILOT      │
                                  │   (Buyer)        │
                                  └────────┬─────────┘
                                           │
                    ┌──────────────────────┴──────────────────────┐
                    │              JOB REQUEST (5932)              │
                    └──────────────────────┬──────────────────────┘
                                           │
                                           ▼
                    ┌──────────────────────────────────────────────┐
                    │              PROVIDER (Pylon)                 │
                    │  ┌─────────────────────────────────────────┐ │
                    │  │           Claude Worker                  │ │
                    │  │  • Analyze issue                        │ │
                    │  │  • Generate patch                       │ │
                    │  │  • Run tests                            │ │
                    │  │  • Log trajectory                       │ │
                    │  └─────────────────────────────────────────┘ │
                    └──────────────────────┬──────────────────────┘
                                           │
                    ┌──────────────────────┴──────────────────────┐
                    │              JOB RESULT (6932)               │
                    │  • patch content                            │
                    │  • patch_sha256                             │
                    │  • trajectory_id                            │
                    │  • test_exit_code                           │
                    │  • bolt11 invoice                           │
                    └──────────────────────┬──────────────────────┘
                                           │
                                           ▼
                    ┌──────────────────────────────────────────────┐
                    │              AUTOPILOT VERIFIES              │
                    │  1. Hash patch content → matches claimed?    │
                    │  2. Apply patch to local repo                │
                    │  3. Run test command → exit 0?               │
                    │  4. Fetch and verify trajectory              │
                    └──────────────────────┬──────────────────────┘
                                           │
                         ┌─────────────────┴─────────────────┐
                         │                                   │
                    PASS │                                   │ FAIL
                         ▼                                   ▼
              ┌──────────────────┐               ┌──────────────────┐
              │  PAY INVOICE     │               │  DISPUTE         │
              │  Update reputation│               │  No payment      │
              └──────────────────┘               └──────────────────┘
```

### 3.2 Verification Criteria

```rust
/// Verification result for a job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Overall pass/fail
    pub passed: bool,
    /// Individual checks
    pub checks: Vec<VerificationCheck>,
    /// Trajectory fetched and validated
    pub trajectory_valid: bool,
    /// Hash of verification evidence
    pub evidence_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationCheck {
    pub name: String,
    pub passed: bool,
    pub details: Option<String>,
}

/// PatchGen verification
impl PatchGenResult {
    pub fn verify(&self, repo_path: &Path, test_command: &str) -> VerificationResult {
        let mut checks = Vec::new();

        // Check 1: Hash matches
        let computed_hash = sha256(&self.patch.as_ref().unwrap_or(&String::new()));
        checks.push(VerificationCheck {
            name: "hash_matches".to_string(),
            passed: computed_hash == self.patch_sha256,
            details: None,
        });

        // Check 2: Patch applies cleanly
        let apply_result = apply_patch(repo_path, &self.patch);
        checks.push(VerificationCheck {
            name: "applies_cleanly".to_string(),
            passed: apply_result.is_ok(),
            details: apply_result.err().map(|e| e.to_string()),
        });

        // Check 3: Tests pass
        if apply_result.is_ok() {
            let test_result = run_command(repo_path, test_command);
            checks.push(VerificationCheck {
                name: "tests_pass".to_string(),
                passed: test_result.exit_code == 0,
                details: Some(format!("exit code: {}", test_result.exit_code)),
            });
        }

        let passed = checks.iter().all(|c| c.passed);
        VerificationResult {
            passed,
            checks,
            trajectory_valid: true, // Checked separately
            evidence_hash: sha256(&serde_json::to_vec(&checks).unwrap()),
        }
    }
}

/// CodeReview verification
impl CodeReviewResult {
    pub fn verify(&self, diff: &str) -> VerificationResult {
        let mut checks = Vec::new();

        // Check 1: Hash matches
        let computed_hash = sha256(&serde_json::to_vec(&self.review).unwrap());
        checks.push(VerificationCheck {
            name: "hash_matches".to_string(),
            passed: computed_hash == self.review_sha256,
            details: None,
        });

        // Check 2: Review is non-trivial
        checks.push(VerificationCheck {
            name: "non_trivial".to_string(),
            passed: !self.review.summary.is_empty()
                && (self.review.issues.len() > 0 || self.review.suggestions.len() > 0),
            details: None,
        });

        // Check 3: File references exist in diff
        let referenced_files: HashSet<_> = self.review.issues.iter()
            .map(|i| &i.file)
            .chain(self.review.suggestions.iter().map(|s| &s.file))
            .collect();
        let diff_files = extract_files_from_diff(diff);
        let all_valid = referenced_files.iter().all(|f| diff_files.contains(*f));
        checks.push(VerificationCheck {
            name: "references_valid".to_string(),
            passed: all_valid,
            details: None,
        });

        let passed = checks.iter().all(|c| c.passed);
        VerificationResult {
            passed,
            checks,
            trajectory_valid: true,
            evidence_hash: sha256(&serde_json::to_vec(&checks).unwrap()),
        }
    }
}
```

### 3.3 Trajectory Logging

Every job produces a trajectory in rlog format (see `crates/recorder/docs/format.md`):

```
---
format: rlog/1
id: job_5932_abc123
repo_sha: def456
provider_pubkey: npub1...
job_type: PatchGen
started_at: 2025-12-31T10:00:00Z
---

@start id=job_5932_abc123 ts=2025-12-31T10:00:00Z
u: Generate patch for issue: "Login button unresponsive"
th: Analyzing the issue description... sig=Ep4E...
t:Read id=call_1 src/components/LoginButton.tsx → [45 lines]
t:Grep id=call_2 pattern="onClick" → [3 matches]
a: Found the issue - event handler not attached correctly.
t:Edit id=call_3 src/components/LoginButton.tsx → [patched]
t:Bash id=call_4 npm test → [exit 0, 12 tests passed]
@end summary="Fixed onClick handler" tokens_in=5000 tokens_out=1200
```

### 3.4 Dispute Resolution

```rust
/// Dispute triggers for agent jobs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisputeTrigger {
    /// Patch doesn't apply
    PatchFailsToApply,
    /// Tests fail after applying patch
    TestsFailAfterPatch,
    /// Hash mismatch
    HashMismatch,
    /// Trajectory missing or invalid
    TrajectoryInvalid,
    /// Job timeout exceeded
    Timeout,
    /// Output doesn't address the issue
    OutputIrrelevant,
    /// Security vulnerability introduced
    SecurityIssue,
}

/// Dispute record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dispute {
    /// Job ID
    pub job_id: String,
    /// Dispute trigger
    pub trigger: DisputeTrigger,
    /// Evidence hash (verification result)
    pub evidence_hash: String,
    /// Buyer's pubkey
    pub buyer_pubkey: String,
    /// Provider's pubkey
    pub provider_pubkey: String,
    /// Status
    pub status: DisputeStatus,
    /// Resolution (if resolved)
    pub resolution: Option<DisputeResolution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisputeStatus {
    Open,
    ProviderResponded,
    UnderReview,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeResolution {
    /// Who won
    pub winner: DisputeWinner,
    /// Reasoning
    pub reasoning: String,
    /// Reputation impact for provider
    pub provider_reputation_delta: i32,
    /// Refund amount (if buyer won)
    pub refund_sats: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisputeWinner {
    Buyer,
    Provider,
    Split,
}
```

**Resolution Process:**

1. Buyer submits dispute with evidence (verification result hash)
2. Provider has 24 hours to respond with counter-evidence
3. Automated checks verify claims against trajectory and artifacts
4. If automated resolution fails, escalate to reputation-weighted arbitrators
5. Resolution affects provider reputation score

---

## 4. Economic Model

### 4.1 Pricing

**Base Pricing by Job Type:**

| Job Type | Base (sats) | Variable | Unit | Cap (sats) |
|----------|-------------|----------|------|------------|
| PatchGen | 3000 | +100 | per file | 20000 |
| CodeReview | 2000 | +50 | per file | 15000 |
| RepoIndex | 500 | +8 | per 1k tokens | 10000 |
| SandboxRun | 200 | +0.5 | per CPU-sec | 5000 |
| TextGen | 0 | +10 | per 1k output | 2000 |

**Provider-Set Pricing:**

Providers advertise their rates via NIP-89, enabling market competition:

```rust
/// Provider's published pricing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPriceBook {
    /// Provider pubkey
    pub provider_pubkey: String,
    /// Pricing per job type
    pub prices: HashMap<AgentJobType, JobPricing>,
    /// Currency (always BTC for now)
    pub currency: String,
    /// Last updated
    pub updated_at: DateTime<Utc>,
}
```

### 4.2 Budget Flow

```
┌──────────────────────────────────────────────────────────────────────┐
│                         BUDGET FLOW                                   │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  USER ($100/month subscription)                                       │
│        │                                                              │
│        ▼                                                              │
│  ┌──────────────────┐                                                │
│  │   AUTOPILOT      │  Converts to sats at current rate              │
│  │   Treasury       │  e.g., $100 → ~100,000 sats                    │
│  │   (Neobank)      │                                                │
│  └────────┬─────────┘                                                │
│           │                                                           │
│           │ Per-job budget allocation                                 │
│           │ e.g., "this issue gets 10,000 sats max"                  │
│           ▼                                                           │
│  ┌──────────────────┐                                                │
│  │   JOB REQUEST    │  Includes max_cost_sats                        │
│  │   (NIP-90)       │  Providers bid against this                    │
│  └────────┬─────────┘                                                │
│           │                                                           │
│           ▼                                                           │
│  ┌──────────────────┐                                                │
│  │   PROVIDER       │  Accepts job, executes                         │
│  │   (Pylon)        │  Returns result + invoice                      │
│  └────────┬─────────┘                                                │
│           │                                                           │
│           │ Invoice for actual_cost <= max_cost                       │
│           ▼                                                           │
│  ┌──────────────────┐                                                │
│  │   VERIFICATION   │  Autopilot verifies result                     │
│  │   (Autopilot)    │                                                │
│  └────────┬─────────┘                                                │
│           │                                                           │
│     ┌─────┴─────┐                                                    │
│     │           │                                                     │
│  PASS          FAIL                                                   │
│     │           │                                                     │
│     ▼           ▼                                                     │
│  PAY        DISPUTE                                                   │
│  INVOICE    NO PAYMENT                                                │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘
```

### 4.3 Settlement

**Lightning invoices with pay-after-verify:**

```rust
/// Payment flow for mesh jobs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshPayment {
    /// Job ID
    pub job_id: String,
    /// Maximum budget allocated by buyer
    pub max_budget_sats: u64,
    /// Invoice from provider
    pub invoice: Option<String>,
    /// Invoice amount (must be <= max_budget)
    pub invoice_amount_sats: u64,
    /// Verification result
    pub verification: Option<VerificationResult>,
    /// Payment status
    pub status: PaymentStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    /// Awaiting job result
    AwaitingResult,
    /// Result received, verifying
    Verifying,
    /// Verification passed, paying
    Paying,
    /// Payment complete
    Paid {
        preimage: String,
        paid_at: DateTime<Utc>,
    },
    /// Verification failed
    VerificationFailed {
        reason: String,
    },
    /// Dispute opened
    Disputed {
        dispute_id: String,
    },
}

/// Payment receipt (for auditing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentReceipt {
    /// Unique receipt ID
    pub receipt_id: String,
    /// Job ID
    pub job_id: String,
    /// Amount paid
    pub amount_sats: u64,
    /// Lightning preimage (proof of payment)
    pub preimage: String,
    /// Trajectory ID (what work was done)
    pub trajectory_id: String,
    /// Verification hash (proof of verification)
    pub verification_hash: String,
    /// Timestamp
    pub paid_at: DateTime<Utc>,
    /// Provider pubkey
    pub provider_pubkey: String,
    /// Buyer pubkey
    pub buyer_pubkey: String,
}
```

### 4.4 Revenue Splits

For jobs involving skills or data from the marketplace:

| Recipient | Share | Notes |
|-----------|-------|-------|
| Compute Provider | 70% | The Claude instance owner |
| Skill Creator | 15% | If job uses a marketplace skill |
| Platform | 10% | OpenAgents network fee |
| Referrer | 5% | If buyer was referred |

```rust
/// Revenue split configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueSplit {
    /// Provider share (basis points, 7000 = 70%)
    pub provider_bps: u16,
    /// Skill creator share (if applicable)
    pub skill_creator_bps: u16,
    /// Platform fee
    pub platform_bps: u16,
    /// Referrer share (if applicable)
    pub referrer_bps: u16,
}

impl Default for RevenueSplit {
    fn default() -> Self {
        Self {
            provider_bps: 7000,      // 70%
            skill_creator_bps: 1500, // 15%
            platform_bps: 1000,      // 10%
            referrer_bps: 500,       // 5%
        }
    }
}
```

---

## 5. Routing & Discovery

### 5.1 NIP-89 Provider Announcements

Providers advertise capabilities via NIP-89 handler info events:

```json
{
  "kind": 31990,
  "pubkey": "<provider_pubkey>",
  "content": "{\"name\":\"Alice's Claude\",\"description\":\"Fast PatchGen\",\"website\":\"...\"}",
  "tags": [
    ["d", "<provider_id>"],
    ["k", "5932"],
    ["k", "5933"],
    ["k", "5931"],

    ["capability", "PatchGen"],
    ["capability", "CodeReview"],
    ["capability", "RepoIndex"],

    ["price", "PatchGen", "3000", "sats", "base"],
    ["price", "PatchGen", "100", "sats", "per_file"],
    ["price", "CodeReview", "2000", "sats", "base"],

    ["capacity", "3"],
    ["isolation", "container"],
    ["model", "claude-sonnet-4-*"],

    ["region", "us-west"],
    ["schedule", "always"],

    ["tier", "2"],
    ["success_rate", "0.97"],
    ["jobs_completed", "523"]
  ]
}
```

### 5.2 Provider Selection

```rust
/// Criteria for provider selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionCriteria {
    /// Job type required
    pub job_type: AgentJobType,
    /// Maximum price (sats)
    pub max_price_sats: u64,
    /// Minimum success rate (0.0 - 1.0)
    pub min_success_rate: f64,
    /// Required isolation level
    pub min_isolation: Option<IsolationMode>,
    /// Preferred regions
    pub preferred_regions: Vec<String>,
    /// Preferred providers (pubkeys)
    pub preferred_providers: Vec<String>,
}

/// Provider score for selection
#[derive(Debug, Clone)]
pub struct ProviderScore {
    /// Provider pubkey
    pub pubkey: String,
    /// Price score (lower is better, normalized 0-1)
    pub price_score: f64,
    /// Reputation score (higher is better, 0-1)
    pub reputation_score: f64,
    /// Capacity score (available/total, 0-1)
    pub capacity_score: f64,
    /// Isolation match (0 or 1)
    pub isolation_match: f64,
    /// Geographic proximity score (0-1)
    pub geo_score: f64,
}

impl ProviderScore {
    pub fn weighted_total(&self, weights: &SelectionWeights) -> f64 {
        self.price_score * weights.price
            + self.reputation_score * weights.reputation
            + self.capacity_score * weights.capacity
            + self.isolation_match * weights.isolation
            + self.geo_score * weights.geo
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionWeights {
    pub price: f64,      // Default: 0.25
    pub reputation: f64, // Default: 0.35
    pub capacity: f64,   // Default: 0.15
    pub isolation: f64,  // Default: 0.15
    pub geo: f64,        // Default: 0.10
}

impl Default for SelectionWeights {
    fn default() -> Self {
        Self {
            price: 0.25,
            reputation: 0.35,
            capacity: 0.15,
            isolation: 0.15,
            geo: 0.10,
        }
    }
}
```

### 5.3 Provider Tiers

| Tier | Requirements | Benefits |
|------|--------------|----------|
| **Tier 0** | <100 jobs, qualification passed | Rate-limited, 5 concurrent max |
| **Tier 1** | 100+ jobs, >90% success | Standard rates |
| **Tier 2** | 500+ jobs, >95% success | 10% price premium allowed |
| **Tier 3** | 1000+ jobs, >99% success | 20% premium, priority routing |

```rust
/// Provider tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ProviderTier {
    Tier0 = 0,
    Tier1 = 1,
    Tier2 = 2,
    Tier3 = 3,
}

impl ProviderTier {
    pub fn from_stats(jobs_completed: u64, success_rate: f64) -> Self {
        if jobs_completed >= 1000 && success_rate >= 0.99 {
            Self::Tier3
        } else if jobs_completed >= 500 && success_rate >= 0.95 {
            Self::Tier2
        } else if jobs_completed >= 100 && success_rate >= 0.90 {
            Self::Tier1
        } else {
            Self::Tier0
        }
    }

    pub fn max_concurrent(&self) -> u32 {
        match self {
            Self::Tier0 => 5,
            Self::Tier1 => 10,
            Self::Tier2 => 25,
            Self::Tier3 => 100,
        }
    }

    pub fn price_premium_allowed(&self) -> f64 {
        match self {
            Self::Tier0 => 0.0,
            Self::Tier1 => 0.0,
            Self::Tier2 => 0.10, // 10%
            Self::Tier3 => 0.20, // 20%
        }
    }
}
```

### 5.4 Fallback Strategies

```rust
/// Strategy for handling provider unavailability
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackStrategy {
    /// Use first available provider
    FirstAvailable,
    /// Prefer specific providers, fallback to pool
    PreferredWithFallback {
        preferred: Vec<String>,
    },
    /// Race multiple providers, use first result
    RaceMultiple {
        count: u32,
    },
    /// Sequential retry with different providers
    SequentialRetry {
        max_attempts: u32,
        backoff_ms: u64,
    },
    /// Require specific provider (no fallback)
    Pinned {
        provider_pubkey: String,
    },
}

impl Default for FallbackStrategy {
    fn default() -> Self {
        Self::SequentialRetry {
            max_attempts: 3,
            backoff_ms: 1000,
        }
    }
}
```

---

## 6. Integration Points

### 6.1 Autopilot as Buyer

Autopilot uses the `MeshClient` to purchase compute:

```rust
/// Autopilot's client for purchasing mesh compute
pub struct MeshClient {
    /// Nostr client for job submission
    nostr_client: Arc<NostrClient>,
    /// Neobank for payments
    treasury: Arc<NeobankService>,
    /// Provider discovery cache
    provider_cache: Arc<RwLock<ProviderCache>>,
    /// Active job tracking
    active_jobs: Arc<RwLock<HashMap<String, MeshJob>>>,
    /// Default selection weights
    selection_weights: SelectionWeights,
    /// Default fallback strategy
    fallback_strategy: FallbackStrategy,
}

impl MeshClient {
    /// Submit a PatchGen job to the mesh
    pub async fn request_patch(
        &self,
        issue: &IssueContext,
        repo: &RepoContext,
        budget_sats: u64,
    ) -> Result<MeshJobHandle, MeshError> {
        // 1. Build request
        let request = PatchGenRequest {
            title: issue.title.clone(),
            body: issue.body.clone(),
            repo_url: repo.url.clone(),
            repo_ref: repo.head_sha.clone(),
            subdir: None,
            allowed_paths: vec!["**".to_string()],
            disallowed_paths: vec![".env".to_string(), "**/*secret*".to_string()],
            test_command: repo.test_command.clone(),
            verification_commands: vec![],
            max_cost_sats: budget_sats,
            time_limit_secs: 900,
            idempotency_key: format!("patch_{}_{}", issue.id, Utc::now().timestamp()),
        };

        // 2. Discover providers
        let providers = self.discover_providers(AgentJobType::PatchGen).await?;

        // 3. Select best provider
        let criteria = SelectionCriteria {
            job_type: AgentJobType::PatchGen,
            max_price_sats: budget_sats,
            min_success_rate: 0.9,
            min_isolation: Some(IsolationMode::Container),
            preferred_regions: vec![],
            preferred_providers: vec![],
        };
        let selected = self.select_provider(&providers, &criteria)?;

        // 4. Submit job
        let job_event = self.create_job_event(&request, AgentJobType::PatchGen)?;
        let job_id = self.nostr_client.publish(job_event).await?;

        // 5. Track and return handle
        let handle = MeshJobHandle::new(job_id.clone(), self.clone());
        self.active_jobs.write().await.insert(job_id, MeshJob {
            request: serde_json::to_value(&request)?,
            provider: selected.pubkey.clone(),
            status: MeshJobStatus::Submitted,
            submitted_at: Utc::now(),
        });

        Ok(handle)
    }

    /// Wait for result and verify
    pub async fn await_and_verify(
        &self,
        handle: &MeshJobHandle,
        timeout: Duration,
    ) -> Result<VerifiedResult, MeshError> {
        // 1. Wait for result event
        let result = self.await_result(&handle.job_id, timeout).await?;

        // 2. Parse result
        let patch_result: PatchGenResult = serde_json::from_str(&result.content)?;

        // 3. Verify
        let verification = patch_result.verify(
            &self.get_repo_path(&handle.job_id)?,
            &self.get_test_command(&handle.job_id)?,
        );

        if verification.passed {
            // 4. Pay invoice
            let receipt = self.treasury.pay_invoice(&patch_result.invoice).await?;
            Ok(VerifiedResult {
                result: patch_result,
                verification,
                payment: Some(receipt),
            })
        } else {
            // 5. Open dispute
            self.open_dispute(&handle.job_id, &verification).await?;
            Err(MeshError::VerificationFailed(verification))
        }
    }
}
```

### 6.2 Runtime /claude Mount

The `/claude` mount's tunnel-based architecture enables agents to delegate work to the mesh:

```rust
/// Delegation from local Claude to mesh
pub struct ClaudeMeshDelegator {
    /// Local Claude session
    local_session: ClaudeSession,
    /// Mesh client for external work
    mesh_client: Arc<MeshClient>,
    /// Delegation policy
    policy: DelegationPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationPolicy {
    /// Enable mesh delegation
    pub enabled: bool,
    /// Maximum cost per delegation (sats)
    pub max_cost_sats: u64,
    /// Job types allowed to delegate
    pub allowed_job_types: Vec<AgentJobType>,
    /// Complexity threshold for delegation
    pub complexity_threshold: Complexity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Complexity {
    Low,
    Medium,
    High,
}

impl ClaudeMeshDelegator {
    /// Decide whether to delegate task to mesh
    pub fn should_delegate(&self, task: &Task) -> DelegationDecision {
        if !self.policy.enabled {
            return DelegationDecision::HandleLocally;
        }

        if task.complexity >= self.policy.complexity_threshold {
            DelegationDecision::DelegateToMesh {
                max_cost: self.policy.max_cost_sats.min(task.budget_sats),
            }
        } else if self.local_session.is_busy() && task.complexity >= Complexity::Medium {
            DelegationDecision::DelegateToMesh {
                max_cost: task.budget_sats / 2,
            }
        } else {
            DelegationDecision::HandleLocally
        }
    }
}

#[derive(Debug, Clone)]
pub enum DelegationDecision {
    HandleLocally,
    DelegateToMesh { max_cost: u64 },
}
```

### 6.3 Neobank Payment Integration

```rust
/// Neobank integration for mesh payments
impl NeobankService {
    /// Pay for verified mesh job
    pub async fn pay_mesh_job(
        &self,
        invoice: &str,
        job_id: &str,
        verification: &VerificationResult,
    ) -> Result<PaymentReceipt, NeobankError> {
        // 1. Decode and validate invoice
        let invoice_details = self.decode_invoice(invoice)?;

        // 2. Check budget
        let job_budget = self.get_job_budget(job_id)?;
        if invoice_details.amount_sats > job_budget {
            return Err(NeobankError::ExceedsBudget {
                invoice: invoice_details.amount_sats,
                budget: job_budget,
            });
        }

        // 3. Create payment with metadata
        let payment = Payment {
            amount_sats: invoice_details.amount_sats,
            invoice: invoice.to_string(),
            metadata: PaymentMetadata::MeshJob {
                job_id: job_id.to_string(),
                verification_hash: verification.evidence_hash.clone(),
                trajectory_id: verification.trajectory_id.clone(),
            },
        };

        // 4. Execute via Lightning
        let preimage = self.lightning.pay(payment).await?;

        // 5. Record and return receipt
        let receipt = PaymentReceipt {
            receipt_id: Uuid::new_v4().to_string(),
            job_id: job_id.to_string(),
            amount_sats: invoice_details.amount_sats,
            preimage,
            trajectory_id: verification.trajectory_id.clone(),
            verification_hash: verification.evidence_hash.clone(),
            paid_at: Utc::now(),
            provider_pubkey: invoice_details.payee_pubkey.clone(),
            buyer_pubkey: self.pubkey.clone(),
        };

        self.record_payment(&receipt).await?;
        Ok(receipt)
    }
}
```

---

## 7. Implementation Roadmap

### Phase 1: Provider Foundation

1. Extend `pylon` with Claude worker pool management
2. Implement PatchGen job handler (kind 5932/6932)
3. Add trajectory logging to worker execution
4. Basic NIP-89 announcements with job types

**Key files:**
- `crates/pylon/src/claude_worker.rs` (new)
- `crates/pylon/src/provider.rs` (extend)
- `crates/compute/src/domain/patch_gen.rs` (new)

### Phase 2: Buyer Integration

1. Add `MeshClient` to autopilot
2. Implement provider discovery and selection
3. Add verification logic for PatchGen
4. Integrate with Neobank for payments

**Key files:**
- `crates/autopilot-core/src/mesh_client.rs` (new)
- `crates/autopilot-core/src/verification.rs` (new)
- `crates/neobank/src/mesh_payments.rs` (new)

### Phase 3: Additional Job Types

1. Implement CodeReview (kind 5933/6933)
2. Enhance SandboxRun with container isolation
3. Add RepoIndex caching and incremental updates

**Key files:**
- `crates/compute/src/domain/code_review.rs` (new)

### Phase 4: Economic Refinement

1. Implement provider tiers and reputation tracking
2. Add dispute resolution automation
3. Implement revenue splits for skills
4. Add USD-denominated budgets via Neobank

### Phase 5: Scale and Polish

1. Geographic routing optimization
2. Advanced fallback strategies (race, fan-out)
3. Provider analytics dashboard
4. Marketplace GUI in HUD

---

## References

- [GTM-claude.md](/home/christopherdavid/code/backroom/live/GTM-claude.md) - Vision document
- [Runtime CLAUDE.md](../crates/runtime/docs/CLAUDE.md) - Tunnel/security model
- [Runtime README](../crates/runtime/README.md) - Runtime architecture
- [NIP-90 Implementation](../crates/nostr/core/src/nip90.rs) - Existing job protocol
- [Autopilot PROJECT-SPEC](autopilot/PROJECT-SPEC.md) - Autopilot phases
- [Marketplace v2](marketplace-v2.md) - Existing NIP-90 compute protocol
