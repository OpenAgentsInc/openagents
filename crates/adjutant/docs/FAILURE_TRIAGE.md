# Failure Triage Integration

This document describes how to wire the existing `FailureTriageSignature` into the autopilot loop to enable intelligent recovery after verification failures.

## Current Gap

### The Problem

1. **SolutionVerifierSignature outputs `next_action`** but it's not used:
   ```rust
   // autopilot/src/dspy_verify.rs:706-718
   let next_action = if verdict == VerificationVerdict::Retry {
       let action = Self::get_string(&prediction, "next_action");
       if action.is_empty() { None } else { Some(action) }
   } else {
       None
   };
   // ← next_action is returned but never consumed
   ```

2. **FailureTriageSignature exists in dsrs** but isn't wired:
   ```rust
   // dsrs/src/signatures/failure_triage.rs
   pub struct FailureTriageSignature { /* ... */ }
   ```

3. **After verification fails, the loop continues blindly:**
   ```rust
   // adjutant/src/autopilot_loop.rs:966-983
   if verification.passed {
       return AutopilotResult::Success(result);
   }
   // else: falls through to retry with no intelligent recovery
   ```

### Symptoms

- Agent retries the same approach that already failed
- No escalation path when stuck
- Burns budget on hopeless retries
- No learning from what went wrong

## Proposed Integration

### Entry Point

Wire failure triage after `SolutionVerifierSignature` returns RETRY or FAIL:

```rust
// crates/adjutant/src/autopilot_loop.rs

async fn run_autopilot_loop(&mut self, task: &Task) -> Result<AutopilotResult> {
    // ... existing planning and execution ...

    // After verification
    let verification = self.verify_completion(&result, iteration).await?;

    match verification.verdict {
        VerificationVerdict::Pass => {
            return Ok(AutopilotResult::Success(result));
        }
        VerificationVerdict::Retry | VerificationVerdict::Fail => {
            // NEW: Call failure triage
            let triage = self.triage_failure(
                &verification,
                &result,
                &current_plan,
            ).await?;

            match triage.next_action {
                TriageAction::FixAndRetry(probe) => {
                    // Insert new step at beginning of plan
                    current_plan.insert_step(0, probe);
                    continue; // Retry with new step
                }
                TriageAction::RetryLarger => {
                    // Increase sandbox resources
                    self.sandbox_profile = SandboxProfile::Large;
                    continue;
                }
                TriageAction::Escalate => {
                    // Ask user for help
                    return Ok(AutopilotResult::NeedsInput(
                        triage.escalation_question
                    ));
                }
                TriageAction::Abort => {
                    return Ok(AutopilotResult::Failed(triage.reason));
                }
                TriageAction::Skip => {
                    // Move to next step, mark current as skipped
                    current_step_idx += 1;
                    continue;
                }
                _ => continue,
            }
        }
    }
}
```

### FailureTriagePipeline

Create a wrapper that combines multiple signals:

```rust
// crates/adjutant/src/dspy/failure_triage.rs (NEW)

use dsrs::signatures::{FailureTriageSignature, FailureCategory, TriageAction};

pub struct FailureTriagePipeline {
    signature: FailureTriageSignature,
    predictor: Predict,
}

impl FailureTriagePipeline {
    pub async fn triage(
        &self,
        verification: &VerificationResult,
        result: &TaskResult,
        plan: &PlanIR,
    ) -> Result<TriageResult> {
        // Gather context from multiple sources
        let last_command = result.last_command();
        let exit_code = result.last_exit_code();
        let stderr = result.last_stderr().truncate(1000);
        let stdout = result.last_stdout().truncate(1000);

        // Include verification analysis
        let test_analysis = verification.test_analysis
            .as_ref()
            .map(|a| a.to_string())
            .unwrap_or_default();
        let build_analysis = verification.build_analysis
            .as_ref()
            .map(|a| a.to_string())
            .unwrap_or_default();

        // Call the signature
        let prediction = self.predictor.forward(example! {
            "command" : "input" => last_command,
            "exit_code" : "input" => exit_code,
            "stderr_preview" : "input" => stderr,
            "stdout_preview" : "input" => stdout,
            "duration_ms" : "input" => result.duration_ms(),
            "test_analysis" : "input" => test_analysis,
            "build_analysis" : "input" => build_analysis,
            "current_step" : "input" => plan.current_step_description(),
        }).await?;

        Ok(TriageResult {
            diagnosis: parse_diagnosis(&prediction)?,
            next_action: parse_action(&prediction)?,
            should_retry: prediction.get("should_retry").as_bool().unwrap_or(false),
            fix_suggestion: prediction.get("fix_suggestion").as_str().map(String::from),
            escalation_question: build_escalation_question(&prediction),
            reason: prediction.get("reasoning").as_str().unwrap_or("").into(),
        })
    }
}
```

### TriageAction → Plan Mutation

| TriageAction | Behavior | When to Use |
|--------------|----------|-------------|
| `FixAndRetry(probe)` | Insert new PlanStep to investigate/fix | Recoverable error with clear next step |
| `RetryLarger` | Increase sandbox resources | OutOfMemory, Timeout with small profile |
| `RetryLonger` | Increase timeout | Timeout but making progress |
| `InstallDependency(dep)` | Add install step | MissingDependency error |
| `UpdateConfig(key, val)` | Add config fix step | ConfigError |
| `Skip` | Mark step as skipped, continue | Non-blocking failure |
| `Escalate` | Ask user for help | Ambiguous situation, needs human judgment |
| `Abort` | Return failure immediately | Unrecoverable error, budget exhausted |

### Example: Inserting a Fix Step

```rust
// When TriageAction::FixAndRetry(probe) is returned:
fn insert_fix_step(plan: &mut PlanIR, probe: ProbeSpec) -> PlanStep {
    PlanStep {
        id: format!("fix-{}", Uuid::new_v4()),
        description: probe.description,
        intent: match probe.probe_type {
            ProbeType::Search => StepIntent::Investigate,
            ProbeType::Edit => StepIntent::Modify,
            ProbeType::Command => StepIntent::Verify,
        },
        target_files: probe.target_files,
        depends_on: vec![], // Runs first
        max_iterations: 2,  // Limit fix attempts
    }
}

// Insert at the beginning
plan.steps.insert(0, insert_fix_step(plan, probe));
```

### Example: Escalation

```rust
// When TriageAction::Escalate is returned:
fn build_escalation_question(triage: &TriageResult) -> String {
    format!(
        "I encountered an issue and need your help:\n\n\
        **Problem:** {}\n\n\
        **What I tried:** {}\n\n\
        **Options:**\n\
        1. Retry with different approach\n\
        2. Skip this step and continue\n\
        3. Abort the task\n\n\
        What would you like me to do?",
        triage.diagnosis.summary,
        triage.fix_suggestion.unwrap_or("No clear fix available".into())
    )
}
```

## Integration Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│  Verification Phase                                                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌──────────────────────┐                                               │
│  │ SolutionVerifier     │                                               │
│  │ Signature            │                                               │
│  └──────────┬───────────┘                                               │
│             │                                                           │
│             ▼                                                           │
│  ┌──────────────────────┐                                               │
│  │ verdict = RETRY?     │───────────────────────────────────────┐       │
│  └──────────┬───────────┘                                       │       │
│             │ NO                                                │ YES   │
│             ▼                                                   ▼       │
│  ┌──────────────────────┐                          ┌───────────────────┐│
│  │ verdict = PASS?      │                          │ FailureTriage     ││
│  └──────────┬───────────┘                          │ Pipeline          ││
│             │                                      └─────────┬─────────┘│
│             ▼                                                │          │
│  ┌──────────────────────┐                                    │          │
│  │ Return Success       │                                    ▼          │
│  └──────────────────────┘                          ┌───────────────────┐│
│                                                    │ TriageAction      ││
│                                                    └─────────┬─────────┘│
│                                                              │          │
│        ┌──────────────────┬──────────────────┬───────────────┼──────────┤
│        │                  │                  │               │          │
│        ▼                  ▼                  ▼               ▼          │
│  ┌───────────┐    ┌───────────┐    ┌───────────┐    ┌───────────┐      │
│  │FixAndRetry│    │RetryLarger│    │ Escalate  │    │  Abort    │      │
│  └─────┬─────┘    └─────┬─────┘    └─────┬─────┘    └─────┬─────┘      │
│        │                │                │                │            │
│        ▼                ▼                ▼                ▼            │
│  ┌───────────┐    ┌───────────┐    ┌───────────┐    ┌───────────┐      │
│  │Insert new │    │Upgrade    │    │Return     │    │Return     │      │
│  │PlanStep   │    │sandbox    │    │NeedsInput │    │Failed     │      │
│  │Continue   │    │Continue   │    │           │    │           │      │
│  └───────────┘    └───────────┘    └───────────┘    └───────────┘      │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## FailureCategory Reference

The `FailureTriageSignature` classifies failures into categories:

| Category | Description | Typical TriageAction |
|----------|-------------|----------------------|
| `OutOfMemory` | Process ran out of RAM | RetryLarger |
| `Timeout` | Process exceeded time limit | RetryLonger or RetryLarger |
| `CompileError` | Code doesn't compile | FixAndRetry (edit code) |
| `TestFailure` | Tests fail | FixAndRetry (fix test or code) |
| `MissingDependency` | Package not installed | InstallDependency |
| `PermissionDenied` | Access denied | Escalate (needs user) |
| `NetworkError` | Network request failed | Skip or Retry |
| `ConfigError` | Configuration issue | UpdateConfig |
| `Unknown` | Unclassified error | Escalate |

## Benefits

1. **Intelligent recovery** - Agent understands what went wrong
2. **Resource management** - Upgrades sandbox when needed
3. **Human-in-the-loop** - Escalates when stuck instead of burning budget
4. **Learning signal** - Triage decisions feed back to training
5. **Plan mutation** - Can insert fix steps dynamically

## File Locations

| Component | File |
|-----------|------|
| FailureTriageSignature | `crates/dsrs/src/signatures/failure_triage.rs` |
| FailureTriagePipeline | `crates/adjutant/src/dspy/failure_triage.rs` (proposed) |
| Integration point | `crates/adjutant/src/autopilot_loop.rs:966` |
| TriageAction enum | `crates/dsrs/src/signatures/failure_triage.rs` |

## See Also

- [dsrs/docs/SIGNATURES.md](../../dsrs/docs/SIGNATURES.md) - FailureTriageSignature definition
- [DSPY-INTEGRATION.md](./DSPY-INTEGRATION.md) - Self-improvement and training
- [autopilot-core/docs/EXECUTION_FLOW.md](../../autopilot-core/docs/EXECUTION_FLOW.md) - Where triage fits in the loop
