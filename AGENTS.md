# OpenAgents Agent Contract (MVP Mode)

## Scope

- This repository is intentionally pruned for MVP work.
- Primary authority is `docs/MVP.md`.
- If guidance conflicts, direct user instruction wins.

## Current Working Set

- Active implementation focus is `crates/wgpui/`.
- Keep `docs/MVP.md` as the product/spec authority.
- Architecture ownership boundaries are defined in `docs/OWNERSHIP.md`.

## Archived Backroom Code

- Most historical code/docs were moved to `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp`.
- Do not pull archived code back by default.
- Restore pieces only when user explicitly directs it.

## Execution Rules

- Before edits, read `docs/MVP.md` and align changes to MVP scope.
- Confirm changes respect `docs/OWNERSHIP.md` crate boundaries.
- Prefer deletion/simplification over expansion unless requested.
- Keep changes small, verifiable, and directly tied to current MVP goals.
- Do not add `.github/workflows/` automation in this repo.

## Implementation Guardrails

- Retained implementation is Rust/WGPUI-first.
- Sync and state continuity must remain deterministic and replay-safe.
- Wallet and payout state must be explicit and truthful in UI and behavior.

## Validation Gates

- Shared dependency hygiene: `scripts/lint/workspace-dependency-drift-check.sh`.
- Architecture boundary hygiene: `scripts/lint/ownership-boundary-check.sh`.
- Clean-on-touch clippy hygiene: `scripts/lint/touched-clippy-gate.sh` with debt allowlist in `scripts/lint/clippy-debt-allowlist.toml`.
- Repo-managed Agent Skills validation: `scripts/skills/validate_registry.sh`.
- Codex chat/skills regression coverage:
  - `cargo test -p autopilot-desktop codex_lane`
  - `cargo test -p autopilot-desktop assemble_chat_turn_input`
  - `cargo test -p codex-client --test skills_and_user_input`
