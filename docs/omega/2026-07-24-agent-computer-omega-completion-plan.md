# Agent Computer completion and Omega cloud capacity plan

- Date: 2026-07-24
- Class: owner-accepted work-packet ledger
- Status: active plan
- Scope: close openagents `#9190` and `#9193`, then make Omega a first-class
  Agent Computer client
- OpenAgents source: `1ca2959fb2f7b8bfdfe5d664adfd9302323de305`
- Omega source: current `OpenAgentsInc/omega` `main`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`

## 1. Outcome

OpenAgents will finish the Sarah Agent Computer epic.
The Agent Computer image will qualify all seven harnesses.
Sarah will keep using live owned cloud capacity with typed fallback.
Omega will then call the same Agent Computer capacity through the shared
runtime seam.

This plan does not invent a second Firecracker control plane.
This plan does not invent a second run store.
This plan does not make Omega the primary Desktop release by itself.

## 2. Open issues under this plan

| Issue | Title | Current state |
| --- | --- | --- |
| [#9190](https://github.com/OpenAgentsInc/openagents/issues/9190) | EPIC: Sarah operational coding on Agent Computer | Open |
| [#9193](https://github.com/OpenAgentsInc/openagents/issues/9193) | Agent Computer image: all seven harnesses | Open |

Closed children that this plan reuses:

| Issue | Result |
| --- | --- |
| [#9191](https://github.com/OpenAgentsInc/openagents/issues/9191) | Sarah dispatches to live `cloud_coding_session`, stops tick spin, proves a real turn |
| [#9192](https://github.com/OpenAgentsInc/openagents/issues/9192) | `@openagentsinc/agent-harness-environment` publishes typed environments |

## 3. Current truth

### 3.1 What already works

- `agent-computer-gce-1` is running on Google Cloud.
- Production arms `CLOUD_CODING_SESSIONS_ENABLED=true` and a live provisioner.
- The Firecracker guest rootfs bake is reproducible and pinned.
- Six harnesses are runtime-qualified with real turns and receipts:
  Pi, OpenCode, Goose, Cursor, Grok, and Claude Code.
- Pi, OpenCode, and Goose use Gemini through runtime-only broker grants.
- Sarah's autonomous tick prefers the owned Agent Computer.
- Live readiness probes replace stale Pylon slot advertisements.
- Typed fallback exists across cloud and owner Pylon lanes.
- Repeat capacity blockers no longer spin every fifteen minutes.

### 3.2 What still blocks `#9193` and `#9190`

1. Codex on Agent Computer still reports
   `executionState: "owner_reauthentication_required"`.
   The isolated Codex credential needs a fresh owner device login.
   That login must never touch the default `~/.codex` home.
2. The image "kept updated" requirement has bake tooling, but no standing
   update cadence or closeout runbook receipt.
3. Keep Codex unqualified and do not close `#9190`.
4. `@openagentsinc/agent-harness-environment` has no production caller yet.
   Sarah still uses a direct dispatch path.
   Omega has no Agent Computer runner yet.

### 3.3 Authority and product laws

- Sarah authority revision 6 prefers the live Agent Computer first.
- A managed-cloud enqueue is not an execution receipt.
- Secrets stay runtime-only.
- No baked provider keys enter the guest image.
- Live capacity evidence is mandatory.
- Zed and Omega own editor and project truth.
- OpenAgents owns work, policy, receipt, and run truth.
- Omega must not create a second Agent Computer control plane.
- GPUI must not become a second durable run authority.

## 4. Target architecture

```text
Sarah tick / Desktop / Omega GPUI
        |
        v
OpenAgents shared runtime or broker
  (Sarah cloud dispatch today; omega-effectd for Omega)
        |
        v
HarnessEnvironment.openagents_cloud
        |
        v
POST /v1/cloud-coding-sessions  (Worker)
        |
        v
oa-cloud-run-bridge -> oa-codex-control
        |
        v
Firecracker Agent Computer guest
  (pinned seven-harness rootfs + turn-runner)
```

Omega consumes Agent Computer only through `omega-effectd`.
Rust supervises the Node service.
Rust does not speak placement or GCE APIs for this product path.

## 5. Packet sequence

Packets AC-01 through AC-04 close the open OpenAgents issues.
Packets HE-01 through HE-02 finish the reusable harness environment.
Packets OMEGA-AC-00 through OMEGA-AC-03 make Omega a real cloud client.

### AC-01: Codex owner re-authentication and qualification

Owning repository: `OpenAgentsInc/openagents`

Work:

- Record the exact owner action in workspace `NEEDS_OWNER.md`.
- Use an isolated Codex home for Agent Computer only.
- Never run `codex login` against the default `~/.codex` home.
- Prove one real Codex coding turn on the current guest image.
- Require staged change, verifier, writeback, exact usage, and cleanup.
- Flip manifest `guestImage.codex.executionState` to qualified.
- Attach turn, artifact, usage, and teardown refs to `#9193`.

Exit:

- Seven of seven harnesses are runtime-qualified.
- Check off the owner action after proof.

Falsifier:

- Manifest still shows `owner_reauthentication_required`.
- Proof uses the default Codex home or a fixture runner.

### AC-02: Image update cadence and `#9193` closeout

Owning repository: `OpenAgentsInc/openagents`

Work:

- Publish a short image-update runbook for rebake and requalify.
- Name the trigger: source change, harness pin change, or timed cadence.
- Keep the pin manifest as the only image truth.
- Roll up all seven harness receipts into one closeout comment.
- Close `#9193` only when the roll-up is complete.

Exit:

- Close `#9193` with seven qualified harnesses and an update policy.
- A later agent can rebake without reconstructing tribal knowledge.

Falsifier:

- `#9193` closes while any harness lacks a real-turn receipt.
- "Kept updated" has no runbook or cadence reference.

### AC-03: Default harness selection policy

Owning repository: `OpenAgentsInc/openagents`

Work:

- After Codex qualification, correct the no-preference default.
- Prefer Codex first when live and ready.
- Fall back through the admitted ordered set on typed refusal.
- Keep explicit `harnessId` selection authoritative when present.
- Add tests for default and fallback selection.

Exit:

- A no-preference managed-cloud turn tries Codex first when ready.
- Live unreadiness still fails closed and rotates by typed reason.

Falsifier:

- Default selection still silently pins OpenCode when Codex is ready.

### AC-04: Close epic `#9190`

Owning repository: `OpenAgentsInc/openagents`

Work:

- Reconcile every success criterion in `#9190` against live receipts.
- Cite `#9191`, `#9192`, `#9193`, AC-01, AC-02, and AC-03 evidence.
- Confirm Sarah still prefers live Agent Computer capacity.
- Confirm fallback and no-spin behavior remain green.
- Confirm authority and redaction invariants remain intact.
- Close `#9190` only after the checklist is complete.

Exit:

- Close `#9190` with an explicit criterion-by-criterion receipt.

Falsifier:

- `#9190` closes without a public checklist against its own success bar.

### HE-01: First production `openagents_cloud` harness runner

Owning repository: `OpenAgentsInc/openagents`

Work:

- Implement one real `HarnessEnvironmentRunner` for `openagents_cloud`.
- Call the existing cloud coding-session route.
- Keep Desktop `desktop_local` behavior unchanged.
- Prefer a shared module that Sarah or Desktop can reuse.
- Prove one non-test caller constructs the typed environment.

Exit:

- The harness-environment package has a real production caller.
- A coding turn through that runner reaches Agent Computer.

Falsifier:

- The package remains published with only test callers.

### HE-02: Released artifact for Omega consumption

Owning repository: `OpenAgentsInc/openagents`

Work:

- Publish or pin an immutable artifact Omega can consume.
- Prefer the existing incubate-then-release path.
- Record version and digest.
- Forbid relative monorepo imports from Omega.

Exit:

- Omega can depend on released bytes only.

Falsifier:

- An Omega packet needs `workspace:*` or a relative openagents path.

### OMEGA-AC-00: Freeze the Omega Agent Computer contract

Owning repository: `OpenAgentsInc/omega` and contract text in `openagents`

Work:

- Freeze Agent Computer as `HarnessEnvironment.openagents_cloud`.
- Freeze that Omega never owns Firecracker, GCE, or placement APIs.
- Freeze that `omega-effectd` is the only Omega mutation path.
- Freeze live-capacity and runtime-only credential rules.
- Place the work under roadmap packet `OMEGA-OA-08`.
- Require `OMEGA-OA-01` before implementation packets.

Exit:

- Owner and assurance accept the freeze.
- Later packets cannot invent a second cloud authority.

Falsifier:

- A GPUI view or Rust crate becomes Agent Computer receipt authority.

### OMEGA-AC-01: `omega-effectd` Agent Computer runner

Owning repository: `OpenAgentsInc/omega`

Work:

- After `OMEGA-OA-01`, add the `openagents_cloud` runner in `omega-effectd`.
- Reuse the released harness-environment artifact from HE-02.
- Project session and turn events over the framed protocol.
- Reuse server-side capacity and fallback truth.
- Do not duplicate `khala-cloud-runtime-dispatch` policy in Rust.

Exit:

- Omega can start and observe an Agent Computer turn through the service.
- Rust only supervises process and protocol health.

Falsifier:

- Omega calls `oa-codex-control` directly from Rust for this product path.

### OMEGA-AC-02: Minimal native command surface

Owning repository: `OpenAgentsInc/omega`

Work:

- Add one bounded GPUI or command entry that starts a cloud turn.
- Show live progress and terminal outcome.
- Keep the surface projection-only.
- Do not wait for Full Auto UI packets.
- Do not create an Omega-only cloud thread store.

Exit:

- An operator can launch one cloud turn from Omega without Full Auto.

Falsifier:

- The only proof path is a hidden test with no operator surface.

### OMEGA-AC-03: Live Omega proof and dual-client closeout

Owning repository: `OpenAgentsInc/omega` with receipts in `openagents`

Work:

- Prove one live Firecracker turn started from Omega.
- Require staged change, verifier, writeback, usage, and teardown.
- Compare receipt shape with the Sarah `#9191` proof class.
- Record that Sarah and Omega share the same capacity authority.
- Link Full Auto later consumption to `OMEGA-FA-*` without blocking this proof.

Exit:

- Omega is a proven Agent Computer client.
- `#9190` remains closed and does not reopen for Omega work.

Falsifier:

- Proof uses a mock cloud runner or omits writeback and cleanup evidence.

## 6. Dependency order

```text
AC-01 Codex re-auth + qualify
  -> AC-02 image cadence + close #9193
  -> AC-03 default harness policy
  -> AC-04 close #9190
  -> HE-01 first openagents_cloud runner
  -> HE-02 released artifact
  -> OMEGA-OA-01 shared runtime seam (roadmap prerequisite)
  -> OMEGA-AC-00 contract freeze
  -> OMEGA-AC-01 omega-effectd runner
  -> OMEGA-AC-02 minimal native surface
  -> OMEGA-AC-03 live Omega proof
```

Safe parallel work:

- Brand and identity Omega packets can continue beside AC-01 through AC-04.
- HE-01 can start after AC-04, or earlier as a non-blocking prototype if it
  does not change default harness policy.
- OMEGA-AC-00 can draft while HE-02 finishes, but cannot implement runners.

Do not parallelize two writers for:

- `agent-computer-image.manifest.json`
- cloud coding-session route contracts
- harness-environment schema
- `omega-effectd` protocol schemas

## 7. Relation to Full Auto and portable execution

This plan lands Agent Computer capacity for Omega under `OMEGA-OA-08`.
It does not complete Full Auto.

Later Full Auto packets may add an `openagents_cloud` lane only after:

1. This plan's OMEGA-AC-03 proof is green.
2. `OMEGA-FA-00` freezes the Full Auto contract.
3. Own-capacity and live-readiness rules remain fail-closed.

Full Auto remains a dedicated run.
Agent Computer remains one execution environment.
Neither absorbs the other.

## 8. Owner action required

AC-01 needs one irreducible owner action:

1. Authenticate an isolated Codex account for Agent Computer.
2. Confirm the login used an isolated home, not `~/.codex`.
3. Allow the qualification turn to proceed.

Record that action in workspace `NEEDS_OWNER.md` before waiting.
Continue AC runbook drafting and HE design while waiting.

## 9. Non-goals

This plan does not:

- reopen closed `#9191` or `#9192` as incomplete
- merge `openagents_cloud` and `managed_sandbox` into one environment
- rebuild Firecracker control in Omega Rust
- bake secrets into the guest image
- trust advertised Pylon capacity without live readiness
- give mobile or GPUI local execution authority
- claim Omega Desktop feature parity or primary cutover
- implement Full Auto launcher packets

## 10. Completion rule

Complete the OpenAgents half after you close `#9193` and `#9190` with the
AC-01 through AC-04 receipts.

The Omega half is complete when OMEGA-AC-03 proves one live Omega-started
Agent Computer turn with writeback and cleanup.

A document alone is not completion.
