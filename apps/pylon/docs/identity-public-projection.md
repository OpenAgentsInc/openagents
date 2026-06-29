# Pylon v0.3 Identity And Public Projection

Status: implemented for `0.3.0-rc1` local bootstrap/status automation.

## Local State

Pylon resolves local state from `PYLON_HOME` or `~/.pylon`.

The managed paths are:

- `config.json` for public-safe bootstrap config;
- `identity.json` for public local identity metadata;
- `identity.mnemonic` for the private NIP-06 mnemonic when an explicit
  `PYLON_HOME` is selected;
- `runtime-state.json` for lifecycle, resource mode, capability refs, and
  blocker refs;
- `ledger.jsonl` reserved for later wallet/settlement events;
- `cache/` and `cache/releases/` for release/update artifacts.

`identity.json` does not contain signing key material. It mirrors only public
identity fields: `nodeId`, `pylonRef`, `nodeLabel`, the 32-byte lowercase hex
Nostr `publicKey`, valid NIP-19 `npub`, and `createdAt`. The mnemonic, `nsec`,
and private hex key stay in the private mnemonic file and must not be projected
or copied into public evidence.

## Nostr Identity

Pylon v0.3 now uses a real NIP-06 Nostr identity for public Nostr fields and
signed Nostr-bound requests. `identity.json` is not the signer. It must not be
used as the source for any private key, `nsec`, relay event signature, Forum
Nostr claim signature, orange-check claim signature, or NIP-98 auth.

The deprecated Rust Pylon that previously lived inside this repo used a real
NIP-06 identity. It stored a BIP39 mnemonic at `identity.mnemonic`, defaulting
to `~/.openagents/pylon/identity.mnemonic`, with these compatibility inputs:

- `OPENAGENTS_IDENTITY_MNEMONIC_PATH` for direct mnemonic-file override;
- historical Pylon config `identity_path`;
- `OPENAGENTS_PYLON_HOME/identity.mnemonic`.

The current implementation checks those historical locations before creating
any new key. When a valid mnemonic exists, it derives the same account-zero
NIP-06 key at `m/44'/1237'/0'/0/0` and projects only public fields. If no
mnemonic exists, it creates a new 12-word BIP39 English mnemonic at the
selected compatibility path with private file permissions. Empty, invalid, or
group/world-readable mnemonic files fail closed instead of silently
regenerating a different identity.

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
