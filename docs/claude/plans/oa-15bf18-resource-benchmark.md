# Plan: Benchmark and Optimize Parallel Agent Resource Limits

**Task ID:** oa-15bf18
**Status:** Planning (not yet executed)
**Goal:** Determine safe resource limits for containerized MechaCoder agents
**Target Host:** 16GB RAM (constrained environment)

## Executive Summary

Create a benchmarking methodology to stress-test parallel MechaCoder agents, find minimum viable RAM/CPU per agent, establish safe concurrency limits, and update defaults to prevent host resource exhaustion.

**Critical constraint:** With only 16GB host RAM, we must determine if 2GB or 4GB per agent is viable, which limits us to 2-4 concurrent agents maximum.

---

## Phase 1: Baseline Measurement

### 1.1 Instrument Resource Monitoring

Create `src/diagnostics/resource-monitor.ts`:
- Use `process.memoryUsage()` for Node/Bun memory
- Shell out to `vm_stat` (macOS) or `/proc/meminfo` (Linux) for system memory
- Use `os.cpus()` and `os.loadavg()` for CPU metrics
- Sample every 5 seconds during agent runs
- Output to `docs/logs/benchmarks/YYYYMMDD-HHMM-resource-baseline.json`

### 1.2 Single Agent Baseline

Run single MechaCoder agent on representative tasks and measure:
- Peak RSS memory
- Average CPU utilization
- Memory over time (detect leaks)
- Container overhead vs bare process

**Test matrix:**
| Task Type | Expected Memory | Measure |
|-----------|-----------------|---------|
| Small fix (single file) | ~500MB-1GB | Actual peak |
| Medium feature (5-10 files) | ~1-2GB | Actual peak |
| Large refactor (20+ files) | ~2-4GB | Actual peak |

---

## Phase 2: Stress Testing

### 2.1 Incremental Concurrency Test

Script: `src/diagnostics/stress-test.ts`

```
For agents in [1, 2, 4, 6, 8]:
  For memory_limit in ["2G", "4G", "6G", "8G"]:
    1. Spawn N agents with memory_limit
    2. Monitor host RAM/swap usage
    3. Monitor per-container memory
    4. Record: completion rate, OOM kills, wall time
    5. Stop if host swap > 50% or OOM detected
```

### 2.2 Memory Pressure Detection

Monitor for:
- Container OOM kills: `container logs` or exit code 137
- Host memory pressure: `vm_stat` page-outs increasing
- Swap usage: `sysctl vm.swapusage`
- Process throttling: completion time degradation >2x

### 2.3 CPU Contention Test

Test with explicit CPU limits:
```
For cpu_limit in [0.5, 1.0, 2.0, "unlimited"]:
  Run 4 agents with cpu_limit
  Measure: completion time, CPU wait time
```

---

## Phase 3: Determine Safe Defaults

### 3.1 Decision Matrix

| Host RAM | Recommended maxAgents | Per-Agent Memory | Safety Margin |
|----------|----------------------|------------------|---------------|
| **16GB** | **2** | **2-4G** | **6-8GB for host** |
| 32GB | 4 | 4G | 16GB for host |
| 64GB | 8 | 4G | 32GB for host |
| 128GB | 12 | 4G | 80GB for host |

Formula: `maxAgents = floor((totalRAM - hostReserve) / perAgentRAM)`

**For 16GB host (primary target):**
- If agent needs 4GB: `(16 - 8) / 4 = 2 agents`
- If agent needs 2GB: `(16 - 8) / 2 = 4 agents` (tight, may swap)
- Benchmark will determine actual minimum RAM requirement

### 3.2 Recommended Defaults

Based on **16GB host** (constrained environment):
- `memoryLimit`: "2G" or "4G" (TBD by benchmark - down from current 8G)
- `cpuLimit`: 1.0 (conservative for constrained host)
- `maxAgents`: 2 (down from 4 - safer for 16GB)
- New: `hostMemoryReserve`: "6G" (ensure host stability on constrained machine)

---

## Phase 4: Implementation

### 4.1 Schema Updates (`src/tasks/schema.ts`)

Add to `SandboxConfig`:
```typescript
// Minimum host memory to reserve (prevents OOM)
hostMemoryReserve: S.optionalWith(S.String, { default: () => "8G" }),
```

Update `ParallelExecutionConfig` defaults:
```typescript
// Auto-calculate maxAgents based on host memory if not specified
maxAgents: S.optionalWith(S.Number, { default: () => undefined }), // auto
```

### 4.2 Auto-Scaling Logic (`src/agent/orchestrator/parallel-runner.ts`)

Add function:
```typescript
const calculateSafeMaxAgents = (
  totalHostMemory: number,
  perAgentMemory: number,
  hostReserve: number
): number => {
  return Math.max(1, Math.floor((totalHostMemory - hostReserve) / perAgentMemory));
};
```

### 4.3 Runtime Validation

Before spawning agents:
1. Check available host memory
2. Warn if requested concurrency exceeds safe limit
3. Optionally auto-reduce `maxAgents` to safe value

---

## Phase 5: Validation

### 5.1 Acceptance Criteria

- [ ] Validated minimum RAM: document actual measured minimum (target: 2-4GB)
- [ ] Defaults updated in `src/tasks/schema.ts`
- [ ] Auto-scaling logic prevents over-allocation
- [ ] 8-hour overnight run with 2 agents completes without host OOM (16GB host)
- [ ] Documentation updated in `docs/mechacoder/MECHACODER-OPS.md`

### 5.2 Test Scenarios

1. **Happy path:** 2 agents, 16GB host, 4G each = stable
2. **Stress test:** 4 agents, 16GB host = should warn/reduce
3. **Edge case:** 16GB host with old defaults (8G each) = must auto-reduce to 1 agent

---

## Files to Modify

| File | Change |
|------|--------|
| `src/tasks/schema.ts` | Add `hostMemoryReserve`, update defaults |
| `src/agent/orchestrator/parallel-runner.ts` | Add auto-scaling logic |
| `src/agent/overnight-parallel.ts` | Add pre-flight memory check |
| `.openagents/project.json` | Update sandbox defaults (memoryLimit: "2G" or "4G") |

---

## Deliverables

1. **This planning doc:** `docs/plans/oa-15bf18-resource-benchmark.md`
2. **Diagnostics tooling:** `src/diagnostics/` (permanent)
   - `resource-monitor.ts` - Host/container memory monitoring
   - `stress-test.ts` - Concurrency stress testing
   - `index.ts` - Module exports
3. **Updated defaults:** Schema changes with validated values for 16GB host
4. **Auto-scaling:** Runtime protection against over-allocation
5. **Documentation:** Update `docs/mechacoder/MECHACODER-OPS.md` with resource guidance

---

## Current State Reference

**Current defaults (to be updated after benchmarking):**

From `src/tasks/schema.ts`:
- `ParallelExecutionConfig.maxAgents`: 4
- `SandboxConfig.memoryLimit`: not set (optional)
- `SandboxConfig.cpuLimit`: not set (optional)

From `.openagents/project.json`:
- `sandbox.memoryLimit`: "8G"
- `sandbox.backend`: "macos-container"

**Key files for implementation:**
- `src/tasks/schema.ts:162-183` - ParallelExecutionConfig
- `src/tasks/schema.ts:139-156` - SandboxConfig
- `src/agent/orchestrator/parallel-runner.ts` - Parallel execution orchestration
- `src/sandbox/macos-container.ts:120-125,201-205` - Container resource flags
