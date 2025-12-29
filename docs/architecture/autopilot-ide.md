# Autopilot IDE Structure (In-Process Runtime + Daemon Interfaces)

## Goals
- Keep `crates/autopilot` as the core engine with no UI dependencies.
- Make the IDE the primary experience while retaining full CLI access.
- Run the Autopilot runtime in-process for low latency UI updates.
- Interface with one or more daemons (autopilotd or future workers) for background execution and monitoring.

## Proposed crate split

| Crate | Type | Responsibility |
| --- | --- | --- |
| `crates/autopilot` | lib | Core Autopilot engine, planning/execution/review, reports, replay, verification. |
| `crates/autopilot-service` | lib | In-process runtime wrapper, shared API surface for CLI + IDE, daemon client traits. |
| `crates/autopilot-ui` | lib | WGPUI components and view-models for the IDE (panels, session lists, tool views). |
| `crates/autopilot-app` | bin | Winit/wgpu app shell that hosts WGPUI root component and connects to services. |

This keeps UI concerns isolated while making Autopilot functionality accessible to both GUI and CLI.

## Data flow overview

```
IDE (autopilot-app)
  -> WGPUI root component (autopilot-ui)
     -> state updates from autopilot-service
        -> in-process Autopilot runtime (autopilot)
        -> daemon clients (autopilotd or other workers)
```

### In-process runtime
- `autopilot-service` owns an `AutopilotRuntime` that wraps `autopilot::StartupState`.
- The IDE ticks the runtime and consumes snapshots for rendering.
- This provides fast, deterministic UI state while avoiding IPC for the common path.

### Daemon interfaces
- `autopilot-service` defines daemon client traits so the IDE can:
  - Poll status and resource usage.
  - Start/stop/restart workers.
  - Subscribe to worker logs or streams.
- This layer also allows multiple daemon backends without changing UI code.

## CLI vs IDE
- CLI commands remain under `openagents autopilot ...` and call into `autopilot-service`.
- IDE is the default interactive experience (`openagents` without args opens `autopilot-app`).
- Both share the same service layer and data model, reducing duplication.

## Implementation notes
- `autopilot-ui` should avoid direct daemon logic; it consumes typed snapshots and emits user intents.
- `autopilot-app` translates intents into service calls and schedules runtime ticks.
- Keep `autopilot-service` free of wgpu/winit dependencies to preserve headless usage.
