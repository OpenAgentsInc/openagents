# OpenAgents Agent Contract (MVP Mode)

## Writing style

Write directly. Prefer blunt declarative sentences over rhetorical scene-setting.

Do not use contrastive metadiscourse or rhetorical throat-clearing such as:

- "the interesting question is not X, it is Y"
- "the real question is..."
- "the point is not..., the point is..."
- "X lives in a different neighborhood"
- "X sits in a different orbit / layer / space"

Do not use vague conceptual metaphors when a direct architectural statement will
do.

Bad:

- "The HyperAgents paper lives in a different neighborhood."
- "The interesting question is not whether this is impressive in the abstract."

Good:

- "HyperAgents improves worker, validator, and routing quality inside OpenAgents."
- "This matters because it lowers cost per accepted outcome."

Default to:

- direct claims
- explicit subsystem names
- concrete causal language
- plain statements of what is true, what changes, and why it matters

## Scope

- This repository is intentionally pruned for MVP work.
- Primary authority is `docs/MVP.md`.
- If guidance conflicts, direct user instruction wins.

## Current Working Set

- The current shipped desktop implementation still uses the existing
  `apps/autopilot-deprecated` and WGPUI paths.
- The current refactor plan for the next Autopilot desktop shell is a Tauri app
  in `apps/autopilot`, not `apps/autopilot-tauri` and not a separate repo by
  default.
- Keep WGPUI as retained infrastructure and specialized rendering unless the
  user asks to remove it.
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
- Do not treat work as issue-complete while it only exists on a feature,
  issue, or temporary worktree branch. Branch work is in-progress evidence.
  An issue can be closed as completed only after the required code/docs are
  merged to and pushed on the canonical default branch (`main`), with any
  required deployment or runtime proof completed from that integrated state.
- Do not add `.github/workflows/` automation in this repo.
- The public `openagents.com` website does not live in this repo. It lives in
  the sibling checkout at `/Users/christopherdavid/work/openagents.com`.
- If the task is about the main website, homepage, landing pages, or public
  domain entry flows, switch repos. Do not edit or recreate
  `apps/openagents.com/` here.

## Local Proof Runtime First

- For distributed training, homework, Nexus authority, Pylon fleet, artifact,
  validator, reconcile, closeout, or payout-proof work, use the local proof
  runtime from `#4385` as the primary development and verification loop.
- Default to `oa proof ...` local runs before any production Nexus deploy,
  host-local probe, Cloudflare tunnel change, or live payout confirmation.
- Production Nexus is a final confirmation surface, not the primary debugger.
  Do not use live Nexus to discover ordinary scheduler, artifact, worker,
  validator, closeout, or payout-eligibility bugs that the local proof runtime
  should be able to reproduce.
- If the local proof runtime cannot reproduce a blocker, improve the proof
  runtime, fixture corpus, transport split view, simulated treasury, artifact
  adapter, or seeded proof lane first. Continue to production only after the
  local harness either reproduces the problem or has a documented, deliberate
  gap that cannot be modeled safely.
- Proof evidence for these issues should include the `oa proof` command,
  namespace, `run-report.json`, `authority-state-trace.json`,
  `proof-summary.json`, first red stage, and any harness gap that had to be
  closed before touching production.
- Use production Nexus only after the relevant local proof lane is green from
  code integrated on `main`, and only to confirm live deployment or real
  settlement behavior that the simulated system intentionally does not cover.

## Nexus Release Process

- There is one production Nexus release path.
- Use a clean `openagents` checkout or temporary worktree at the exact commit
  being shipped.
- Build and push the registry image with:
  `bash scripts/deploy/nexus/01-build-and-push-image.sh`
- Deploy that registry image with:
  `DEPLOY_IMAGE=... bash scripts/deploy/nexus/03-configure-and-start.sh`
- Verify the same deployment with:
  `DEPLOY_IMAGE=... bash scripts/deploy/nexus/04-verify-gates.sh`
- After deploy, also verify `https://nexus.openagents.com/v1/treasury/status`
  and any task-specific payout or receipt checks required by the change.
- Do not bypass this path with VM-local `docker build`, VM-local image tags,
  manual systemd drop-ins, or ad hoc `docker run` replacements on
  `nexus-mainnet-1`.
- If the registry build or scripted deploy is blocked, report the blocker.
  Do not leave production pinned to a VM-local image as the steady state.

## Psionic Specs

- Psionic now lives in the standalone `OpenAgentsInc/psionic` repo.
- Normal `openagents` Cargo builds fetch Psionic from pinned git dependencies,
  so a separate local Psionic clone is not required just to run
  `cargo autopilot`, `cargo check`, or `cargo test` here.
- If you need cross-repo dev or retained validation scripts, clone Psionic as a
  sibling checkout:
  `git clone https://github.com/OpenAgentsInc/openagents.git && git clone https://github.com/OpenAgentsInc/psionic.git`
  so `openagents` sees `../psionic` by default.
- When working on Psionic from this repo, use that repo’s `docs/ARCHITECTURE.md`
  as the canonical Psionic-wide system spec for runtime, cluster, datastream,
  sandbox, serving, artifact, receipt, failure, and security boundaries.
- When working on training-class Psionic work, use the standalone repo’s
  `docs/TRAIN_SYSTEM.md` as the canonical train-system spec for current
  substrate, planned train architecture, object model, receipts, policy
  surfaces, failure semantics, and issue-program direction.
- Cross-repo validation scripts in `openagents` assume a local Psionic checkout
  at `../psionic` unless `OPENAGENTS_PSIONIC_REPO` is set.
- Repo-local Cargo builds use the vendored `PROTOC` wrapper in
  `.cargo/config.toml`, so transitive protobuf builds should not require a
  machine-local `protoc` install.

## Programmatic Control And Test Docs

- The running desktop app now has an app-owned control plane plus a thin CLI:
  - implementation: `apps/autopilot-deprecated/src/desktop_control.rs`
  - CLI: `apps/autopilot-deprecated/src/bin/autopilotctl.rs`
- The narrow user/agent-facing guide for the current internal Forge CLI lives
  in `docs/codex/AUTOPILOTCTL_FORGE_CLI.md`.
- The agent-specific runbook for how to operate shared Forge sessions lives in
  `docs/codex/FORGE_AGENT_INSTRUCTIONS.md`.
- Forge also has a no-window host at
  `apps/autopilot-deprecated/src/bin/autopilot_headless_forge.rs`, and
  `autopilotctl forge ...` will autostart that host when the resolved
  desktop-control target is missing or stale.
- Full operator/testing docs for `autopilotctl`, headless compute, packaged
  app roundtrips, and log artifacts live in `docs/headless-compute.md`.
- The current release-cut/product-scope truth, including the seller-first role
  of buyer-side smoke-test flows, lives in `docs/v01.md`.
- When changing programmatic control, packaged verification, or Spark/NIP-90
  roundtrip behavior, update those docs if the behavior or contract changed.

## WGPUI Screenshot Capture (agent)

When the user asks for screenshots, visual review artifacts, or proof that a
WGPUI surface actually renders, use the repo-native capture lane before trying
OS-level screenshot tooling.

Prefer repo-native capture when:

- the target is a WGPUI example, storybook section, viz primitive, or
  component showcase
- the user wants reviewable PNGs after a visual change
- the target is an app-owned pane and a typed desktop-control export exists
- determinism matters more than whatever happens to be visible on screen

Do not default to:

- `screencapture`
- `osascript` window automation
- ad hoc manual screenshots of a visible app window

Read these first:

- `docs/wgpui/HEADLESS_CAPTURE.md`
- `docs/headless-compute.md`
- `scripts/wgpui/capture-gallery.sh`
- `scripts/wgpui/capture-storybook.sh`
- `crates/wgpui/examples/gallery_capture.rs`
- `crates/wgpui/examples/storybook_capture.rs`
- `apps/autopilot-deprecated/src/desktop_control.rs`
- `apps/autopilot-deprecated/src/bin/autopilotctl.rs`

Useful commands:

```bash
scripts/wgpui/capture-gallery.sh \
  --target viz-primitives \
  --output target/wgpui-captures/review/viz-primitives.png

scripts/wgpui/capture-gallery.sh \
  --target component-showcase \
  --output target/wgpui-captures/review/component-showcase.png

scripts/wgpui/capture-storybook.sh \
  --section Autopilot \
  --output target/wgpui-captures/review/storybook-autopilot.png

OPENAGENTS_DISABLE_CODEX=1 cargo run -p autopilot-desktop \
  --bin autopilot_headless_data_market -- \
  --manifest-path /tmp/openagents-pane-capture.json \
  --bind 127.0.0.1:0

cargo run -p autopilot-desktop --bin autopilotctl -- \
  --manifest /tmp/openagents-pane-capture.json \
  pane capture \
  --output /tmp/psionic-remote-training-pane.png \
  psionic_remote_training
```

Current pane-capture truth:

- `autopilotctl pane capture` is the typed app-owned screenshot export path
- the first honest supported pane target today is `psionic_remote_training`
- pane capture writes both a PNG and a JSON manifest beside it

Image review:

- if the user wants the image opened locally on this Mac, run
  `open /absolute/path/to/file.png`
- if the agent needs to inspect the generated image inside the transcript, use
  the image-view tool on the absolute local path
- always report the exact output path back to the user

Expected workflow:

1. Prefer gallery or storybook capture for WGPUI-owned components.
2. Use pane capture only when the user specifically needs a real app-owned
   pane.
3. After generating a PNG, inspect it yourself before reporting success.
4. When visual changes land, offer fresh capture artifacts if that is the
   clearest verification.

## Data Market Docs (agent)

When the task is about packaging local material, publishing a listing/grant,
running seller or buyer flows, consuming a delivery, or explaining how the Data
Market works, read these docs first and in this order:

1. `docs/v02.md`
   - user-facing release doc for the Data Market MVP launch
   - explains what exists in UI vs `autopilotctl` vs
     `autopilot_headless_data_market` vs repo-owned skills
   - includes explicit guidance for how to point agents at the right surface,
     path boundary, manifest, packaged artifact, or delivery selector
2. `docs/kernel/markets/data-market.md`
   - canonical implementation/status doc for the current Data Market
   - defines the kernel truth objects (`DataAsset`, `AccessGrant`,
     `DeliveryBundle`, `RevocationReceipt`)
   - records the current NIP-90/NIP-89 kind numbers, packaging helpers,
     CLI/headless shape, code ownership, and repo entrypoints
3. `docs/headless-data-market.md`
   - exact no-window and `autopilotctl data-market ...` runbook
   - covers packaging, draft/preview/publish, payment, delivery, revoke, and
     `consume-delivery`
   - documents local `nexus-control` setup, manifest targeting, and the local
     plus public-relay E2E harnesses
4. `skills/README.md`
   - registry/index for the three first-party Data Market skills:
     `autopilot-data-seller`, `autopilot-data-market-control`, and
     `autopilot-data-seller-cli`
5. `docs/PANES.md`
   - current behavior/ownership of the `Data Seller`, `Data Market`, and
     `Data Buyer` panes when UI semantics matter

For concrete, already-proven flows, use these audits as operator truth:

- `docs/audits/2026-03-18-headless-data-market-publish-consume-audit.md`
  - local headless publish -> request -> delivery -> consume proof
- `docs/audits/2026-03-18-public-relay-data-market-publish-consume-audit.md`
  - strict live public-relay proof on Damus + Primal
- `docs/audits/2026-03-18-data-market-paid-buyer-access-audit.md`
  - real paid buyer-access proof and the current Nostr vs control-plane split
- `docs/audits/2026-03-18-psionic-intro-data-market-sale-audit.md`
  - concrete example of packaging and listing a sample dataset for sale
- `docs/audits/2026-03-21-data-seller-one-sentence-prompt-paid-flow-audit.md`
  - proven one-sentence `Data Seller` prompt -> publish -> paid buy ->
    delivery -> consume flow using local `nexus-control`

## Data Market Entry Points (agent)

Do not invent a second publication path and do not tell agents to vaguely click
around the app. Use the repo’s existing entrypoints:

- UI seller flow:
  - `Data Seller` pane
  - skill: `skills/autopilot-data-seller/SKILL.md`
- Typed app-owned tool flow:
  - skill: `skills/autopilot-data-market-control/SKILL.md`
  - use `openagents.data_market.*` tools and read authority state back after
    every mutation
- Shell-first / no-window packaging + publication flow:
  - skill: `skills/autopilot-data-seller-cli/SKILL.md`
  - runtime: `apps/autopilot-deprecated/src/bin/autopilot_headless_data_market.rs`
  - CLI: `apps/autopilot-deprecated/src/bin/autopilotctl.rs`
  - conversational terminal automation of the same seller lane:
    `autopilotctl data-market seller-prompt "<prompt>"`

Current packaging helpers and seller wrappers:

- deterministic package helper:
  `scripts/autopilot/data_market_package.py`
- Codex conversation redaction/package helper:
  `scripts/autopilot/package_codex_conversations.py`
- seller CLI wrappers:
  - `skills/autopilot-data-seller-cli/scripts/package_data_asset.sh`
  - `skills/autopilot-data-seller-cli/scripts/package_codex_conversations.sh`

Current verification / proof scripts:

- `scripts/autopilot/headless-data-market-smoke.sh`
- `scripts/autopilot/headless-data-market-e2e.sh`
- `scripts/autopilot/headless-data-market-public-e2e.sh`
- `scripts/autopilot/verify-data-market-cli-headless.sh`

Current operating truth:

- preview-only seller work can run without kernel authority, but real
  asset/grant publish requires a control/authority endpoint
- the normal local MVP path is `nexus-control` plus a desktop session token via
  `OA_CONTROL_BASE_URL` and `OA_CONTROL_BEARER_TOKEN`
- Nostr currently carries the targeted request / feedback / result transport,
  while the control plane / kernel authority owns the canonical
  asset / grant / delivery / revocation state

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

## Linux GPT-OSS bring-up (agent)

When working on autopilot, Mission Control, or seller-mode bring-up on a supported Linux NVIDIA host, run the app with the GPT-OSS env vars set so Mission Control can select the CUDA lane and auto-warm the configured GGUF.

1. **Set the backend**: `export OPENAGENTS_GPT_OSS_BACKEND=cuda`
2. **Set the model path**: `export OPENAGENTS_GPT_OSS_MODEL_PATH=/absolute/path/to/gpt-oss-20b-mxfp4.gguf`
3. **Default model path**: if `OPENAGENTS_GPT_OSS_MODEL_PATH` is unset, the runtime looks for `~/models/gpt-oss/gpt-oss-20b-mxfp4.gguf`
4. **Run the desktop app**: `cargo autopilot` (or `cargo run -p autopilot-desktop --bin autopilot-desktop --`)
5. **Verify the local runtime**: `autopilotctl local-runtime status` and `autopilotctl wait local-runtime-ready`
6. **Then bring the seller lane online**: `autopilotctl provider online`

Mission Control on the Linux GPT-OSS lane now auto-warms the configured model at startup and on go-online preflight when the CUDA backend and GGUF artifact are present. If you change env vars or swap GGUFs during a session, run `autopilotctl local-runtime refresh` and wait for `local-runtime-ready` again.

## Validation Gates

- Shared dependency hygiene: `scripts/lint/workspace-dependency-drift-check.sh`.
- Architecture boundary hygiene: `scripts/lint/ownership-boundary-check.sh`.
- Clean-on-touch clippy hygiene: `scripts/lint/touched-clippy-gate.sh` with debt allowlist in `scripts/lint/clippy-debt-allowlist.toml`.
- Repo-managed Agent Skills validation: `scripts/skills/validate_registry.sh`.
- GPT-OSS parity regression gate: `scripts/lint/gpt-oss-parity-gate.sh`.
- Codex chat/skills regression coverage:
  - `cargo test -p autopilot-desktop codex_lane`
  - `cargo test -p autopilot-desktop assemble_chat_turn_input`
  - `cargo test -p codex-client --test skills_and_user_input`
- Programmatic packaged roundtrip:
  - `scripts/release/check-v01-packaged-autopilotctl-roundtrip.sh`
- Supported NVIDIA/CUDA Mission Control smoke:
  - `scripts/release/check-gpt-oss-nvidia-mission-control.sh`
