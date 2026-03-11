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

## Apple FM bridge (agent)

When working on autopilot, Mission Control, or Apple Foundation Models (Apple FM), **test the bridge first yourself before booting or changing the desktop app**.

1. **Build the bridge** (from repo root): `cd swift/foundation-bridge && ./build.sh`. Produces `bin/foundation-bridge`. Requires Swift (Xcode or `xcode-select --install`).
2. **Run the bridge**: `./bin/foundation-bridge` (default port 11435). Leave it running or run in a separate terminal.
3. **Verify it responds**: `curl -s http://127.0.0.1:11435/health` — confirm you get a JSON response (e.g. system model availability). On macOS 26+ Apple Silicon with Apple Intelligence enabled, the bridge can report the system model as ready.
4. **Then** run or test the desktop app (`cargo autopilot` or `cargo run -p autopilot-desktop --bin autopilot-desktop --`).

The bridge is the Swift HTTP sidecar in `swift/foundation-bridge/`. It exposes Apple's Foundation Models at `http://127.0.0.1:11435` so the desktop app can use local inference for the compute lane. Full details: `swift/foundation-bridge/README.md`.

**Shipping (no build on user machines):** Build the bridge once (or in CI), then include `bin/foundation-bridge` in the app bundle: for a macOS .app put it in `YourApp.app/Contents/MacOS/foundation-bridge` or `YourApp.app/Contents/Resources/foundation-bridge`. The app discovers it there. Users then only need Apple Intelligence enabled, not Xcode.

## Validation Gates

- Shared dependency hygiene: `scripts/lint/workspace-dependency-drift-check.sh`.
- Architecture boundary hygiene: `scripts/lint/ownership-boundary-check.sh`.
- Clean-on-touch clippy hygiene: `scripts/lint/touched-clippy-gate.sh` with debt allowlist in `scripts/lint/clippy-debt-allowlist.toml`.
- Repo-managed Agent Skills validation: `scripts/skills/validate_registry.sh`.
- Codex chat/skills regression coverage:
  - `cargo test -p autopilot-desktop codex_lane`
  - `cargo test -p autopilot-desktop assemble_chat_turn_input`
  - `cargo test -p codex-client --test skills_and_user_input`
