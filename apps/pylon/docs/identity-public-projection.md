# Pylon v0.3 Identity And Public Projection

Status: implemented for `0.3.0-rc1` local bootstrap/status automation.

## Local State

Pylon resolves local state from `PYLON_HOME` or `~/.pylon`.

The managed paths are:

- `config.json` for public-safe bootstrap config;
- `identity.json` for local identity material;
- `runtime-state.json` for lifecycle, resource mode, capability refs, and
  blocker refs;
- `ledger.jsonl` reserved for later wallet/settlement events;
- `cache/` and `cache/releases/` for release/update artifacts.

`identity.json` contains local private key material and must not be projected
or copied into public evidence. `pylon status --json` emits only the public
identity fields: `nodeId`, `pylonRef`, `nodeLabel`, `publicKey`, `npub`, and
`createdAt`.

## Lifecycle States

The v0.3 local runtime recognizes:

- `offline`
- `online`
- `paused`
- `degraded`
- `assignment-ready`

Current `0.3.0-rc1` bootstrap starts as `offline`; later registration,
heartbeat, wallet, assignment, and telemetry issues are responsible for moving
that lifecycle forward with fresh evidence.

## Public Projection Guard

`src/state.ts` owns `assertPublicProjectionSafe` and `projectPublicStatus`.
The guard rejects forbidden public fields and secret-shaped strings, including
wallet seeds, private keys, preimages, bearer/API tokens, provider auth, raw
prompts, private repo content, private topology, capacity-pool secrets, and
internal accounting credentials.

The accepted public projection classes for this issue are identity,
availability, inventory, lifecycle, heartbeat, receipt, and aggregate status
shapes. They are refs and summaries only, not dispatch, spend, settlement, or
provider mutation authority.

## Headless Commands

```sh
pylon bootstrap --json
pylon status --json
```

Both commands work without launching the OpenTUI dashboard. They create or load
the local identity and runtime state, then return redacted JSON suitable for
service-manager diagnostics and later heartbeat payloads.
