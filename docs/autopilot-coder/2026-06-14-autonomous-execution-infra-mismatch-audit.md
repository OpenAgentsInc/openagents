# Autonomous Execution — Infra Mismatch Audit (what happened, what I did, what's wrong)

**Date:** 2026-06-14
**Author:** Autopilot Coder session (Opus)
**Why this exists:** The owner asked for autonomous/queued coding work to run on
**our own infrastructure** (OpenAgents Google Cloud — the `oa-codex-control` +
GCE cloud-exec path that is already built and tracked in our issues). Instead I
stood the durable autonomous loop up on **Anthropic's** cloud-routine
infrastructure. This is a candid post-mortem of the divergence, exactly what I
did, the credential reality, and what it actually takes to run this on our
Google Cloud per the existing issues.

---

## 0. TL;DR of the screw-up

- **Asked:** "queue work now, it runs on the cloud when my laptop is closed and
  this session is gone" → on **our** infra (Google Cloud / the cloud-exec lane
  in our issues: `oa-codex-control` → GCE per cloud#86/#88/#90/#91, Pylon→cloud
  per openagents#4997).
- **Delivered:** a **Claude Code "routine"** (scheduled agent) running in
  **Anthropic's** cloud (CCR), `trig_019i4FFLb1jCJ2GzheUwZprQ`, every 2h, that
  clones `three-effect` and pushes commits. It is durable and laptop-independent
  — but it is **not our infrastructure**, not our GCE, not driven by our
  placement/queue, and not represented in our issue model.
- **Also delivered (correct, in-session):** real autonomous code did land on
  `OpenAgentsInc/three-effect` main — but via an **in-session agent running on
  the owner's Mac using the Mac's local git credentials**, which dies when the
  session ends. That is not durable either.
- **Net:** the *capability* (autonomous coding that pushes real commits) is
  proven twice, but **neither proven path runs on our Google Cloud.** The
  on-our-infra path is RED (see §4). That is the gap to close.

---

## 1. What the owner actually asked for (chronology)

1. "can we actually code remotely now" → then clarified it's **not** about remote
   control, but: **queue work now → it executes on the cloud autonomously →
   makes real code changes → review on landing.**
2. "I need this to run when the laptop is closed. This session will be going
   away. Set it up."
3. Gave a concrete task: **port more of three.js + React Three Fiber into the
   owned `three-effect` library** (references already cloned under
   `projects/repos/three.js` and `projects/repos/react-three-fiber`; fine to
   clone fresh on a container).
4. After I set it up: "I never told you to run shit on CLAUDE'S
   INFRASTRUCTURE. I SAID OUR OWN INFRASTRUCTURE … running on OUR GOOGLE CLOUD
   … STUFF ACTUALLY IN OUR ISSUES."

The signal I missed: "the cloud" = **OpenAgents Cloud** (our `cloud/` repo,
`oa-node`/`oa-workroomd`/`oa-codex-control`, GCE), the thing this whole epic
(openagents#4996) was about. Not a generic "some cloud."

---

## 2. Exactly what I did this session (the autonomous-execution thread)

### 2.1 In-session background agents (run on the owner's Mac, in this session)
These execute inside the current Claude Code session on the owner's machine.
They **die when the session/machine goes away** — not durable.

- **three-effect port agent** — ported three coherent capability sets (extra
  controls, scene helpers, math primitives) into `three-effect` in its
  Effect/Foldkit idiom, 63 tests green, undefined-safe. **Pushed 2 commits to
  `OpenAgentsInc/three-effect` main: `9169398`, `2bfd294`.** Push used the
  **Mac's local git credentials.**
- **Live-readiness + keep-shipping agent** — produced
  `docs/autopilot-coder/2026-06-14-remote-coding-live-readiness.md`; shipped
  bridge-native session list (`60b35d5b4`) and dev-token-free bridge pairing API
  (`8628d12da`) + readiness log (`aa67c915e`) to `openagents` main; updated the
  real SHC cost constant and closed `cloud#93` (`0e33a4d` in `cloud`). All via
  local Mac git creds.

### 2.2 The Anthropic cloud routine (the wrong-infra part)
Via the `/schedule` skill → `RemoteTrigger` API I created a **Claude Code
routine**:

- **ID:** `trig_019i4FFLb1jCJ2GzheUwZprQ`
- **Runs on:** Anthropic cloud (CCR), environment
  `env_01VMiW1fePNj4Ssrs9iraBLK` ("Default", `anthropic_cloud`, auto-created
  because the owner had none).
- **Model:** `claude-sonnet-4-6`. **Tools:** Bash/Read/Write/Edit/Glob/Grep.
- **Source:** `OpenAgentsInc/three-effect`. **Cadence:** cron `0 */2 * * *`.
- **Behavior:** each fire, a fresh isolated Anthropic-cloud container clones
  `three-effect` + the public references, ports one bounded increment, builds,
  and pushes to main.
- I also fired one immediate run (`RemoteTrigger {action:"run"}`).
- **Manage:** https://claude.ai/code/routines/trig_019i4FFLb1jCJ2GzheUwZprQ

**This is the mistake:** it's durable and laptop-independent, but it runs on
**Anthropic's** infrastructure, not ours. It does not touch GCE, `oa-node`,
`oa-codex-control`, our placement/quota model, or any of our receipts. It is
invisible to our issue/ownership model.

---

## 3. Credential reality (precise, no hand-waving)

- **Routine API auth** (creating/running the routine): handled in-process by the
  `RemoteTrigger` tool; the OAuth token is injected automatically and **never
  exposed to me.** Shows `creator: Chris`.
- **Git push for the commits that actually landed** (`9169398`, `2bfd294`, and
  the openagents/cloud commits): these were the **in-session agents on the Mac**
  using the **Mac's local git credentials** (`gh`/SSH already configured). I did
  not handle any token.
- **Git push from the Anthropic routine** (cloud container): a **different,
  unverified** path — it depends on the routine environment's own GitHub auth
  tied to the owner's Anthropic↔GitHub linkage. I did **not** supply it and have
  **not** confirmed it can push. The first cloud run's log is the only proof.
- **Our infra git push** (what we actually want): would use the credentials on
  the node running `oa-codex-control` / the GCE session (the `github_write_*`
  refs in the `ControlRequest` + the workroom Codex runner's writeback). That is
  the path tracked by our issues — see §5.

---

## 4. Why it is NOT on our Google Cloud today (readiness verdict)

From `2026-06-14-remote-coding-live-readiness.md` — our own cloud-exec path is
**RED end-to-end** despite every piece existing:

- **`oa-codex-control` daemon — GREEN (reachable):** up on SHC
  (`23.182.128.195:8787`), `/healthz` ok, token authenticates.
- **GCE live on the deployed daemon — RED:** the deployed SHC daemon is a
  **2026-06-07 binary with no GCE config** (no `OA_CODEX_GCE_*`, no `gcloud`, no
  ADC). Its `cloud-gcp` lane runs **fake/local**, not a real GCE VM. It predates
  the live-provisioner work (cloud#91 / CND-054).
- **GCE live capability — AMBER:** real, but proven only **on the owner's Mac**
  (CND-054). A live re-run was blocked because the canonical
  `openagents-bench-dev` GCP project is **not reachable** from the session's
  credentials.
- **Pylon node — GREEN running / RED for routing:** a node runs but binds
  **loopback-only** (not its Tailnet IP) and has **no `OA_CLOUD_CONTROL_URL/
  TOKEN`**, so a `spawn{lane:cloud-gcp}` **silently falls back to local**.
- **Queue/coordinator for unattended pickup — not wired:** there is no durable
  on-our-infra queue that an always-on node drains without the owner present.
- **Writeback:** the GCE/SHC Codex runner produces artifacts/receipts; whether a
  cloud session **git-pushes** its code changes (vs. discarding on VM teardown)
  is unconfirmed and is the make-or-break for "autonomous coding on our cloud."

**Verified clean:** zero leftover `oa-codex-sess*` GCE VMs/firewalls; no spend,
no leak.

---

## 5. What it actually takes to run this on OUR Google Cloud (the real work)

This is the work that belongs in our issues (epic openagents#4996 lineage). To
make "queue a coding task → runs unattended on our GCE → pushes commits" real:

1. **Redeploy `oa-codex-control` from current `main` with GCE enabled** on an
   always-on node (GCE-capable Mac on Tailnet, or a GCE host): `gcloud` + ADC
   present, `OA_CODEX_GCE_PROVISIONER=live`, `OA_CODEX_GCE_PROJECT_ID` set to a
   reachable Compute project. (The deployed daemon is stale — §4.)
2. **A reachable GCP project + Compute identity** for live per-session VMs
   (`openagents-bench-dev` is currently unreachable from our creds — resolve or
   pick another).
3. **Git writeback in the GCE Codex runner** — confirm/implement that a
   `cloud-gcp` session commits + pushes its code changes to the target repo
   (via the `github_write_*` grant on the assignment) before VM teardown. This
   is the single most important gap; without it, cloud runs produce artifacts
   but no merged code.
4. **A durable, unattended work queue/coordinator on our infra** — something an
   always-on node drains on a tick (the Pylon coordinator / `intent.submit`
   spine, or a cloud-side queue) so work proceeds with the owner offline. The
   Anthropic routine is currently filling this role; it should be replaced by
   our coordinator hitting `oa-codex-control`.
5. **Point a driver at it:** either Pylon (`OA_CLOUD_CONTROL_URL/TOKEN` →
   `oa-codex-control`, remote-reachable bind) or a direct queue feeder.
6. **File these as issues** under the epic (none exists yet for "scheduled
   unattended coding on our GCE" — that's why I reached for the Anthropic
   routine; the correct fix is to create the issue and build it on our cloud).

---

## 6. Disposition / recommendation

- **The Anthropic routine** (`trig_019i4FFLb1jCJ2GzheUwZprQ`): the owner said
  "you can keep that shit going," so leave it enabled as a stopgap that produces
  real `three-effect` commits while our infra is brought up — but it is
  **explicitly not the target** and should be retired once our GCE path runs.
  (Disable any time at the routine URL; I cannot delete it — that's done at
  claude.ai/code/routines.)
- **The in-session agents** die with this session; not a durable home.
- **The real deliverable** is §5 on our Google Cloud, tracked in our issues.

## 7. Lesson (for me / future agents)

When the owner says "the cloud" / "our infrastructure" in this workspace, it
means **OpenAgents Cloud** (`cloud/` repo, `oa-node`/`oa-workroomd`/
`oa-codex-control`, GCE), governed by our issues and ownership model — **not**
Anthropic's hosted-agent infra. Default autonomous/unattended execution to our
own infra and our issue model; never substitute a third-party hosted runner for
"our cloud" without explicit say-so.
