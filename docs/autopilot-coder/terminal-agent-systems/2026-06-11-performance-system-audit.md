# Performance System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #48 from the Bun/Effect terminal-agent systems list. It defines
how the terminal agent should measure and manage latency, throughput,
responsiveness, memory use, tool execution time, model streaming, and
long-running background work.

## Target

Build a performance system that keeps interactive turns responsive, background
work observable, and runtime degradation diagnosable without exposing private
payloads.

## User-Visible Capability

Users should be able to:

- See when a run is slow because of model latency, tool execution, queueing,
  provider limits, workspace setup, or verification.
- Continue typing while background tasks run.
- Cancel or pause slow work.
- Inspect a redacted performance profile.
- Know whether rate limits, budget caps, or local resource pressure caused a
  stop.
- Avoid UI stalls while large outputs stream.

The terminal should degrade gracefully under long logs, large diffs, slow
providers, and expensive searches.

## Metrics Model

Track:

- Turn admission latency.
- Model first-token and completion latency.
- Tool proposal and execution duration.
- Shell command wall time and output volume.
- File read/edit size.
- Context assembly time and token count.
- Event-log append and projection time.
- Queue depth.
- Memory and disk pressure.
- Adapter and provider retry counts.

Metrics should be tagged with run refs and coarse classes, not raw prompt or
payload content.

## Bun/Effect Boundary

Use Effect services for:

- `PerformanceMetricsService`: records spans and counters.
- `BackpressureService`: controls queues, streams, and rendering pressure.
- `RuntimeProfileService`: exports redacted profiles.
- `ResourceLimitService`: enforces memory, output, and time limits.
- `PerformanceProjectionService`: shows user-safe run diagnostics.

Use Stream for large output with bounded buffering. Use Queue for work
scheduling. Use Schedule for retries and periodic samples. Use Scope for span
lifetime.

## Safety Rules

- Performance traces cannot include raw private payloads.
- Slow rendering cannot block cancellation.
- Output truncation must preserve full private artifact refs where allowed.
- Timeouts create typed failure events.
- Budget stops and performance stops are distinct.
- Background queue pressure should be visible before runs silently starve.
- Profiling must be opt-in when it could expose local paths or command detail.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has token/cost accounting, provider-account lease
state, Pylon host inventory, and run smokes, but the terminal-agent README does
not yet include a performance system audit.

Related open issue anchors:

- #4766 account-pool dashboard needs lease load and cooldown timing.
- #4767 rate-limit rotation proof.
- #4768 overnight unattended proof.
- #4770 team budgets and spend-to-evidence join.

No claim should say the terminal runtime is production-performance ready until
latency, queueing, output-volume, and cancellation behavior are measured.

## Tests

Minimum coverage:

- Render large streamed output without UI stalls.
- Cancel while output is streaming.
- Enforce tool, shell, model, and context timeouts.
- Distinguish provider rate limit from local performance pressure.
- Export redacted profiles.
- Apply backpressure to event and output streams.
- Recover from queue overload with visible blocker refs.
- Preserve artifact refs when display output is truncated.

## Decision

Performance should be a first-class runtime system with private-safe spans,
limits, and projections. It should not be inferred only from final success or
failure.

