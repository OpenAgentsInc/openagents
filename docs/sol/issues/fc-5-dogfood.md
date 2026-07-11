# P0 PROOF: accepted simultaneous Codex + Claude Phase A

- Issue: #8640
- Program parent: #8566
- Depends on: closed #8637/#8633/#8639 Fleet substrate
- Status: active bounded owner-authorized live proof
- Authority: [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md), Revision 31
- Child repairs: closed #8685 Claude owner-local permission authority; #8686
  supervisor lifecycle and verifier/publication ordering, implementation
  landed at `d98abda795` with the live parent receipt pending

## Outcome

Produce one accepted real simultaneous FleetRun with useful Codex and Claude
work through explicitly named isolated owner accounts. This issue proves the
mixed runtime substrate only. Desktop/mobile observation, control, fault
convergence, and sustained product dogfood remain owned by #8574/#8597/#8676/
#8677/#8566 and do not keep #8640 open after Phase A.

No Fleet run is authorized merely because this issue is open. Each live attempt
still requires the owner's explicit authorization and a fresh bounded claim.

## Current live truth

- C1 is crossed: the durable run, mixed supervisor, command/reconnect, evidence,
  and closeout substrate is closed and deployed.
- One named isolated Codex registry account is live-proven ready. Default
  `~/.codex` is forbidden for automatic work.
- The Worker/Pylon ambient type boundary and strict SCM scanner corrections are
  landed. Selected provider homes skip only their own login patterns; SCM/PAT/
  Forge/extraheader detection remains strict.
- FleetRun `fleet_run.fc5.mixed.phase-a.20260710t2212z` proved simultaneous
  named `codex` plus named `claude-pylon-3` execution with no default-home use,
  but both closeouts were rejected: Claude reported
  `claude_agent_execution_refused`; Codex used the wrong verifier entry point.
- The broader Khala test exposed a leaked supervisor-scope failure. CUT-06
  repairs it at `d98abda795`: cancellation reaches Codex/Claude execution, the
  supervisor loop joins before its guard releases, concurrent restart is
  fenced, and verifier/correlation evidence precedes terminal publication.
  Focused, full Pylon, and integrated deploy gates pass. This deterministic
  repair is not the still-required simultaneous accepted proof.
- CUT-05 is closed at `509fb27ea1`: a real named `claude-pylon-3` assignment
  completed with an accepted closeout under a revocable, exact-scope,
  process-opaque owner-local grant. Public, bridge, org-cloud, replayed,
  expired, revoked, and mismatched paths remain bounded. This repairs the
  Claude refusal but is not the simultaneous parent receipt.
- Grok is not an acceptance item while funded capacity is unavailable.

## Scope

1. Preserve the closed CUT-05 exact-scope Claude authority and its restrictive
   public/remote defaults.
2. Preserve the landed supervisor-scope cleanup and verifier/publication
   ordering under the production-path receipt.
3. Bind each useful work unit to one correct package-script/single-argv verifier
   before dispatch.
4. Run at least two simultaneous pinned public work units under one FleetRun:
   one named Codex account and one named Claude account.
5. Use the authenticated typed adapter and perform one steer or approval round
   trip.
6. Preserve Pylon-owned post-verification credential scanning and publication;
   workers do not commit, push, open/modify PRs, or mutate issue comments.

The child issues close the known code defects. #8640 still requires its own
accepted simultaneous named-account live receipt before the parent closes.

## Acceptance

- Codex and Claude each produce useful verified work and one accepted closeout.
- There are zero duplicate claims, default account homes, silent provider
  substitutions, or manually launched per-assignment shells.
- Verification, post-run scan, publication, and accepted closeout occur in the
  enforced order and survive reconnect.
- Exact provider usage is recorded when available; otherwise the receipt says
  `not_measured`. Failed/rejected attempts do not fabricate spend or completion.
- One typed steer or approval reaches exactly one durable outcome.
- The evidence bundle records pinned source/deployment/app versions, run/work/
  attempt/claim/session/account refs, latency/freshness/stalls, exact verifier,
  scan result, usage truth, public-safe artifact refs, and could-not-prove list.

No fixture, synthetic harness, default provider home, substituted provider, or
rejected closeout satisfies acceptance.

## Explicit transfer

The former #8640 body also required R3 two-client control and R7 sustained
dogfood. Those are real program exits but made this proof an epic. They now
remain explicitly in:

- #8676 — first live Desktop conversation and physical mobile continuation;
- #8677 — command/event fault convergence;
- #8574/#8597 — Fleet projection/control and client release tracks; and
- #8566 — sustained R7 product dogfood and legacy retirement.

## Close

Close immediately after the accepted Phase A receipt and honest evidence bundle
are posted. Do not wait for full Desktop/mobile product cutover, and do not call
Phase A alone that cutover.
