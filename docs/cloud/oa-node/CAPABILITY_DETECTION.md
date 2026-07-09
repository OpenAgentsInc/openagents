# oa-node Capability Detection

Status: implemented for Cloud MVP issue `CND-008`

`oa-node detect --json` emits a local capability detection report. The report
separates hardware that is present on the host from capacity that the managed
Cloud node may honestly sell.

## Detection Shape

The detection report includes:

- host facts projected into `openagents.cloud_node.v1`;
- present hardware: OS, architecture, CPU, memory, disk, and accelerator hints;
- sellable capability rows with `present_hardware`, `backend_ready`, and
  `eligible`;
- degraded backend reasons.

The MVP currently detects host hardware with `sysinfo`. Psionic and sandbox
rows are intentionally not eligible until their backends and profiles are
configured and probed.

## Status Projection

After `oa-node init`, `oa-node status --json` projects detected host facts and
capability rows into `openagents.cloud_node.v1`.

Backend failures or missing backend configuration make the relevant capability
`backend_ready=false` and `eligible=false`; they do not advertise sellable
capacity.

## Verification

```bash
cargo test -p oa-node --test capability_detection
cargo run -p oa-node -- detect --json
```
