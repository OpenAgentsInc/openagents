# CAST First-Class Support Plan

Date: 2026-03-03
Status: Planning

## 1. Goal

Add first-class support for Charms CAST workflows in OpenAgents so an agent can reliably execute the end-to-end DEX lifecycle (create order, cancel/replace order, partial fulfill, inspect/verify) with deterministic tooling, safety checks, and verifiable outputs.

This plan is based on all files in:

- `/Users/christopherdavid/code/charms/cast-releases/docs/howto/README.md`
- `/Users/christopherdavid/code/charms/cast-releases/docs/howto/02-cancel-and-replace-order.yaml`
- `/Users/christopherdavid/code/charms/cast-releases/docs/howto/03-partial-fulfill.yaml`
- `/Users/christopherdavid/code/charms/cast-releases/docs/howto/prev_txs_phase2.txt`
- `/Users/christopherdavid/code/charms/cast-releases/docs/howto/prev_txs_phase3.txt`

## 2. What "First-Class" Means Here

For this repo, "first-class support" means CAST is treated as a dedicated, supported integration surface (not a one-off prompt recipe), with:

1. A standalone CAST skill in `skills/` with explicit metadata, workflow, and references.
2. Reusable CAST scripts for prereq checks and deterministic execution primitives.
3. Canonical docs/runbooks under `docs/charms/` for operator and agent usage.
4. Validation and test coverage aligned with existing skills-registry and lint gates.
5. Clear safety constraints around signing keys, private inputs, and broadcast steps.

## 3. Scope Boundaries (MVP + Ownership)

In scope:

- Skill-level support and execution workflow for CAST on Bitcoin with Charms.
- Deterministic local automation patterns for proving/signing/broadcasting CAST spells.
- Docs and runbooks for repeatable CAST operations.

Out of scope for this phase:

- New marketplace UI or CAST-specific product pane in `apps/autopilot-desktop`.
- Broad protocol changes in `crates/wgpui` or `crates/spark`.
- Any non-Bitcoin chain support.

Boundary alignment:

- Keep CAST integration in skill/docs/script surfaces unless app-level UX is explicitly requested later.
- Do not introduce app-specific business logic into reusable crates.

## 4. Source-Derived Requirements (from CAST how-to)

Required supported operations:

1. Create order (ask at minimum).
2. Cancel and replace order atomically with maker cancellation signature.
3. Partially fulfill order with correct remainder linkage.
4. Decode and verify resulting spell transaction.

Required external tooling:

- `charms`
- `bitcoin-cli`
- `jq`
- `curl`
- `envsubst`
- `scrolls-nonce`
- `sign-txs`
- `cancel-msg`

Required external services and artifacts:

- Scrolls API (`/address`, `/sign`)
- Cast contract Wasm artifact (`charms-cast-v0.2.0.wasm` or pinned successor)
- Operator-signed fulfill params
- Previous transaction hex ancestry (`prev_txs`)

Required invariants to validate in workflows:

- Scrolls nonce/address derivation correctness.
- Cancellation message/signature format: `{utxo_id} {outputs_hash}`.
- Partial-fill remainder invariants (`exec_type.partial.from`, same maker/side/asset/price, same Scrolls address).
- Fee math and minimum output conventions.
- Spell/public/private input correctness before prove.

## 5. Deliverables

## 5.1 Skill Package

Create new skill namespace:

- `skills/cast/SKILL.md`
- `skills/cast/references/*.md`
- `skills/cast/scripts/*.sh`
- Optional `skills/cast/assets/*.yaml` templates

Skill document responsibilities:

- Define CAST-specific workflow lanes (maker, taker, cancel/replace, inspection).
- Pin required commands and environment contracts.
- Provide safe command templates for check/mock/prove/sign/broadcast.

## 5.2 Documentation

Create docs under `docs/charms/`:

- `CAST_FIRSTCLASS_SUPPORT_PLAN.md` (this file)
- `CAST_OPERATOR_RUNBOOK.md`
- `CAST_TEST_MATRIX.md`
- Optional: `CAST_FAILURE_MODES.md`

## 5.3 Scripts

Planned scripts (initial set):

1. `check-cast-prereqs.sh`
- Modes: `maker`, `taker`, `cancel`, `server`
- Verifies command availability and Bitcoin RPC reachability.

2. `derive-scrolls-address.sh`
- Inputs: `funding_utxo_id`, `output_index`, `scrolls_base_url`
- Output: nonce + address, machine readable.

3. `cast-spell-check.sh`
- Runs `charms spell check` with validated env inputs.

4. `cast-spell-prove.sh`
- Runs mock prove and real prove with explicit input files.

5. `cast-cancel-signature.sh`
- Wraps `cancel-msg message` + `cancel-msg sign` with safe input handling.

6. `cast-sign-and-broadcast.sh`
- Applies wallet and Scrolls signatures and performs controlled broadcast.

7. `cast-show-spell.sh`
- Uses `charms tx show-spell --json` for post-broadcast verification.

## 5.4 Templates

Provide parameterized spell templates:

- `create-ask-order.template.yaml`
- `cancel-replace-order.template.yaml`
- `partial-fulfill-order.template.yaml`
- Optional follow-on: `create-bid-order.template.yaml`

Template requirements:

- Placeholder names must be explicit and stable.
- No embedded secrets.
- Compatible with `envsubst` or explicit JSON rendering.

## 6. Execution Plan (Phased)

## Phase 0: Contract + Environment Lock

Tasks:

1. Pin CAST contract release and hash policy.
2. Define environment variable contract for all scripts.
3. Decide network policy defaults (`mainnet` vs `testnet4` for iteration).
4. Define artifacts directory layout (`inputs/`, `proofs/`, `signed/`, `receipts/`).

Exit criteria:

- One documented environment contract and artifact layout adopted across docs/scripts.

## Phase 1: CAST Skill Scaffolding

Tasks:

1. Add `skills/cast/SKILL.md` with CAST-specific overview/workflow/quick commands.
2. Add references for:
- order lifecycle
- cancellation and signatures
- partial fill invariants
- signing and broadcast paths
3. Add prereq check script and wire into reference usage.
4. Register skill in `skills/README.md`.

Exit criteria:

- Skill validates via `scripts/skills/validate_registry.sh`.
- Skill is discoverable by existing local skills registry logic.

## Phase 2: Deterministic Workflow Scripts

Tasks:

1. Implement script wrappers for check/prove/sign/broadcast/inspect.
2. Enforce strict input validation and consistent JSON outputs.
3. Add dry-run mode for each mutation step.
4. Include explicit confirmation gates before broadcast operations.

Exit criteria:

- A complete maker and partial-fill path can be executed via scripts without manual command rewriting.

## Phase 3: Safety, Determinism, and Failure Handling

Tasks:

1. Add guardrails for secret handling:
- no private key echo
- no secret values in logs
- redact sensitive env vars in debug output
2. Add failure taxonomy and remediation table (RPC, prover, Scrolls signing, mempool rejection).
3. Add deterministic receipt bundle format (inputs used, tx hex produced, txids, verification output).

Exit criteria:

- Every run yields a reproducible receipt and clear error category on failure.

## Phase 4: Verification and Test Matrix

Tasks:

1. Define CAST test matrix in `docs/charms/CAST_TEST_MATRIX.md`:
- positive: create, cancel/replace, partial fulfill
- negative: bad cancel sig, wrong scrolls nonce, stale prev_txs, invalid fee assumptions
2. Add script smoke checks for CI/local lint path (non-destructive checks only).
3. Verify skills registry and ownership boundary gates still pass.

Exit criteria:

- Test matrix green for required scenarios.
- No ownership-boundary regressions.

## Phase 5: Optional Product-Layer Enhancements (Later)

Not part of immediate first-class skill support, but planned extension path:

1. Desktop command presets for CAST tasks.
2. CAST operation receipts surfaced in activity feed.
3. Wallet-linked CAST trade history visualization.

Exit criteria:

- Explicitly scoped as app-layer work, requested separately.

## 7. Environment Contract (Draft)

Core environment keys:

- `CAST_NETWORK` (`mainnet|testnet4`)
- `CAST_SCROLLS_BASE_URL`
- `CAST_APP_BIN` (path to cast wasm)
- `CAST_OPERATOR_PARAMS_FILE` (JSON/YAML payload)
- `CAST_PREV_TXS_FILE`
- `CAST_FUNDING_UTXO`
- `CAST_FUNDING_UTXO_VALUE`
- `CAST_CHANGE_ADDRESS`
- `CAST_FEE_RATE`
- `CAST_MEMPOOL_BROADCAST_URL`

Signing-related keys (prefer file inputs over env where possible):

- `CAST_CANCEL_XPRV_FILE` (if required for cancellation signing)
- `CAST_CANCEL_DERIVATION_PATH`
- `BITCOIND_CONTAINER` (optional passthrough for `sign-txs`)

Policy:

- Secrets must not be committed.
- Scripts must fail fast when required values are missing.

## 8. Data and Artifact Conventions

Working directory shape per run:

- `run/<timestamp>/inputs/`
- `run/<timestamp>/rendered/`
- `run/<timestamp>/proofs/`
- `run/<timestamp>/signed/`
- `run/<timestamp>/receipts/`

Receipt JSON minimum fields:

- `operation` (`create_order|cancel_replace|partial_fill`)
- `network`
- `spell_file`
- `prev_txs_hash`
- `mock_check_passed`
- `prove_output_tx_hex[]`
- `signed_tx_hex[]`
- `broadcast_txids[]`
- `spell_decode_summary`
- `timestamp_utc`

## 9. Risks and Mitigations

1. External dependency drift (`scrolls-nonce`, `sign-txs`, `cancel-msg`, CAST wasm)
- Mitigation: version pinning and preflight version checks.

2. Operator params rotation / signature mismatch
- Mitigation: strict params file validation and clear mismatch errors.

3. Unsafe key handling in cancellation flow
- Mitigation: xprv file-based inputs, shell tracing disabled, redaction policy.

4. Non-deterministic broadcast outcomes
- Mitigation: explicit dry-run + signed artifact persistence + retry policy docs.

5. Incomplete prev_txs ancestry causing prove failure
- Mitigation: ancestry helper checks and targeted failure messages.

## 10. Acceptance Criteria

First-class CAST support is complete when all are true:

1. A dedicated `skills/cast` package exists and validates.
2. CAST workflows are documented with executable, deterministic scripts.
3. Required operations work end-to-end with reproducible receipts:
- create order
- cancel/replace order
- partial fulfill
4. Failure modes are categorized with operator remediation steps.
5. Lint and boundary gates remain green.

## 11. Proposed Task Breakdown (Implementation Order)

1. Create CAST skill skeleton + references.
2. Add prereq and scrolls derivation scripts.
3. Add spell check/prove wrappers.
4. Add cancellation signature wrapper.
5. Add sign+broadcast wrapper and inspection wrapper.
6. Write operator runbook and failure-mode guide.
7. Build and execute test matrix.
8. Finalize docs and release checklist.

## 12. Immediate Next Step

Start Phase 1 by adding `skills/cast/` and CAST references/scripts, then validate via:

- `scripts/skills/validate_registry.sh`
- relevant script smoke checks (non-broadcast by default)
