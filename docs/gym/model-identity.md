# Model content identity

Kev publishes `artifact_identity` and `execution` on every card returned by
`GET /v1/models`. Gym records the content digest as
`door_identity.artifact_signature` and retains the execution settings
separately. An adapter replacement at the same path, with the same public
model name and base revision, produces a different recorded identity.

## What Kev hashes

The runtime reads each input once. The same byte buffer is hashed and
decoded, so the digest identifies the loaded input even if the file later
changes. Input buffers are released after decoding; identity does not
retain a second copy of the weights.

The inputs are:

- Base `config.json` and each loaded `model*.safetensors` shard.
- Adapter configuration and `adapter_model.safetensors`.
- The converted `head.safetensors` and `head_meta.json`.
- `tokenizer.json`, which contains the tokenizer the Rust runtime uses.

An absent optional `head_meta.json` has an explicit null entry; it is
different from an existing file. An unreadable file is an error. Raw
`head.pt` and auxiliary tokenizer files are acquisition inputs, but the
Rust runtime does not load them. Their hashes remain in the
[artifact lock](../kev/artifacts.md).

The identity schema is `openagents.kev.artifacts.v1`. Each logical filename
maps to its SHA-256 and byte count. The digest hashes the compact UTF-8 JSON
array `[schema, files]`, with filenames sorted and each file record ordered
as `sha256`, then `bytes`. Local paths and public model names are excluded.
The published digest is prefixed with `sha256:`.

`execution` records backend, compute dtype, pointer-head dtype, attention
implementation, LoRA merge order, option isolation, and state/branch token
bounds. Numerical changes can therefore keep the checkpoint identity while
changing the measurement identity.

## Discovery, comparisons, and calibration

Gym selects the card matching the client's requested model ID, name, or
advertised alias. It does not attribute every model in a bundle to the
first card. For an older door that publishes one card, that card remains
the fallback; an unresolved name in a multi-model listing stays unknown.

The result store includes the complete door identity in its trial key.
Two checkpoints can answer the same item without becoming duplicate trials.
`gym compare` separates newly identified checkpoints and execution
configurations, even when their door names match. Its comparison label adds
a digest of the complete identity. If a baseline name matches several
sides, supply the full comparison label. `gym regress` refuses an identity
change and names checkpoint or execution changes in its explanation.

A calibration record must match both the checkpoint digest and execution
settings. A historical Kev record that names only a base revision is
incomplete and cannot serve as a calibration match. Kev requires a valid
content digest plus reported dtype and backend. Existing Lev records keep
their base-signature and adapter checks; this change adds no Lev admission.

## Record versions and migration

New result rows use `openagents.gym.eval_row.v3`. Readers still accept
v1 and v2 rows without filling in missing content identity or rewriting
their receipts. New calibration records use
`openagents.gym.calibration_record.v2`; v1 records remain readable and
usable when their existing identity is sufficient for the target door.

The new version tags make old readers refuse new records. Without a
version change, an older typed reader could discard the content digest and
silently compare two adapters as the same base model. Keep historical
stores and maps intact. Record a fresh, identified pass when new evidence
is needed; a path or variant name cannot reconstruct missing provenance.

## What verification establishes

The artifact downloader checks bytes against reviewed immutable locks.
The runtime content digest identifies what was actually loaded, whether or
not a lock was present. Model discovery reports the serving process's claim;
Gym does not independently read remote weights, authenticate the process,
or prove that a named upstream revision supplied those bytes.

The legacy `verified` field means identity evidence is reported, not that
Gym performed remote attestation. Calibration also checks completeness:
old Kev rows can retain their historical `verified: true` field while
remaining incomplete for a new calibration match.

Regression tests replace an adapter at the same path, discover both models
through the Jev client, and retain both identities in a receipt-verified
Gym store. Additional tests cover path-independent identity, changes after
loading, absent metadata, model aliases, comparison grouping, legacy
decoding, and calibration refusal after checkpoint or dtype changes.

On 2026-09-20, the pinned manual gate passed formatting, both strict
Clippy configurations, both workspace test configurations, Rust 1.95
workspace and Rust 1.94 Kev checks, and dependency policy. PostgreSQL store
acceptance passed; gateway acceptance stopped at an existing socket-reset
assertion in `gateway_postgres.rs:829`. Later PostgreSQL stages did not run,
so this is partial gate coverage. The routine worktree run had no external
weights or Swift helper. A separate release run against the pinned,
recovered 0.5B bundle passed all Kev tests, including six conformance tests
and four Jev-client HTTP tests. Metal and soak were not run for identity.
