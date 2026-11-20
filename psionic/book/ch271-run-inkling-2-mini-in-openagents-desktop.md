# 271. Run Inkling 2 Mini in OpenAgents Desktop

*Status: implemented (bounded OpenAgents Desktop readiness, hardware preflight, and selectable model-picker gating).*  
*Source: [`psionic/lib/inkling-desktop.coax`](../lib/inkling-desktop.coax), [`psionic/examples/inkling-desktop.coax`](../examples/inkling-desktop.coax).*

## What this chapter builds

Part XXVII culminates in making Inkling 2 Mini a first-class, selectable local model inside OpenAgents Desktop (`apps/openagents-desktop/`):

1. **Hardware preflight and residency gating** (`inkling_desktop_status`): requires Apple Silicon with ≥32 GB unified memory (`INKLING_MIN_RAM_MB = 32768`) and a verified ~20 GB local checkpoint before allowing selection in the composer model picker.
2. **Real-time install/download state**: models four states (`not_installed`, `downloading`, `installed`, `unsupported_hardware`) with concise status summaries (`inkling_desktop_summary`) surfaced in Settings > Models and the Local Models manager.
3. **OpenAgents Desktop integration**: wires Inkling 2 Mini into the Desktop model picker (`apps/openagents-desktop/page/models.coax`) and Local Models management UI (`apps/openagents-desktop/settings.coax`, `apps/openagents-desktop/Window+Settings.swift`).

## Runnable program

```jai
#load "../lib/inkling-desktop.coax";

main :: () {
  ready := inkling_desktop_status(32768, true, false, 100);
  low := inkling_desktop_status(16384, true, false, 100);
  missing := inkling_desktop_status(65536, false, false, 0);
  print("% (%): selectable=%\n", INKLING_MODEL_LABEL, inkling_desktop_summary(ready), ready.selectable);
  print("16GB Mac: % (selectable=%)\n", inkling_desktop_summary(low), low.selectable);
  print("64GB Mac before download: % (selectable=%)\n", inkling_desktop_summary(missing), missing.selectable);
}
```

Expected output (`psionic/fixtures/inkling-desktop.out`):

```text
Inkling 2 Mini (Local) (Installed (~20 GB)): selectable=true
16GB Mac: Requires 32 GB RAM (selectable=false)
64GB Mac before download: Not downloaded (~20 GB) (selectable=false)
```

## Verification

- `coax/bin/coax test psionic/lib/inkling-desktop.coax` (18 checks covering ch266–ch271).
- `coax/bin/coax run psionic/examples/inkling-desktop.coax`.
