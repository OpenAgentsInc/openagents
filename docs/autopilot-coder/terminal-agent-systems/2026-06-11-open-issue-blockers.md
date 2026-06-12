# Open Issue Blockers

Date: 2026-06-11

Source set: #4749, #4766-#4783, #4785-#4786, and #4813-#4823, as
delegated by
`docs/autopilot-coder/terminal-agent-systems/2026-06-11-open-issue-delegation-plan.md`.

This file records blockers and deferred proof items from the current issue
bodies and comments. It is not a claim on any agent profile.

## Active Blockers

### #4749 W3 student-program evaluation

Status: open, not part of the Autopilot MVP door-open gate.

Blockers:

- Baselines (a), (b), and (c) remain incomplete. The run was interrupted at
  about 12 percent training.
- The four-baseline report has not been published.
- H1/H2/H3 verdicts have not been recorded against their falsifiers.
- Work should resume only under the now-binding CPU-budget policy from the
  referenced psionic incident.

Unblocked evidence already recorded:

- The 103.6M-token verified corpus snapshot exists and exceeds the 100M-token
  input bar.
- Baseline (d) is complete with reported pass@1 and replay acceptance evidence.

### #4767 M9 live rate-limit rotation proof

Status: open.

Blockers:

- The CI-safe deterministic rotation leg is landed.
- Live fleet readiness and live failover route receipts were recorded on
  2026-06-12 for `chris@openagents.com`: two new ChatGPT/Codex accounts
  connected and sanity-checked healthy, an initial lease selected
  `provider-account_ref_2dd6a8b25aad4d93a42947bec62c8465`, and a
  live-induced `rate_limited` failover produced receipt
  `provider_account_failover_receipt_c5f60166e739403799b2291c076d9801`
  with next lease `provider-account-lease_ref_fd9a4982d0d34a54a07bcf72dadc46f0`
  on `provider-account_ref_7b41e0634ec743b6a4855379b3e0fb18`.
- The replacement lease was released and active lease count returned to zero.
- The first account is intentionally on timed cooldown until
  `2026-06-12T03:25:55.212Z`; until cooldown clears or another account is
  connected, only one account is currently eligible for selection.
- The remaining acceptance gap is mission continuity: a real Autopilot run must
  resume under the rotated lease and attach mission/artifact refs showing
  post-rotation work builds on pre-rotation state.

Next unblock:

- After cooldown clears or another account is connected, run a real mission
  through the same failover path and attach the mission continuity evidence
  from steps 4-6 of `docs/autopilot-coder/rate-limit-rotation-smoke.md`.

### #4768 M10 overnight unattended proof

Status: open.

Blockers:

- No live overnight unattended proof receipts exist yet.
- Lane A still needs an SHC scheduled/background run that completes or surfaces
  a decision, notifies, is reviewed, and has correct metering.
- Lane B still needs the same unattended flow on an own cloud/local Pylon with
  the laptop closed and zero credits debited.
- Both runs must be visible from `pylon work status` and the web UI with
  matching states and refs.
- A full receipt trail, clean redaction scan, and promise/registry evidence are
  still required.
- The Gate proof authority scaffold is merged, but it is evidence-only and does
  not substitute for the live run.

Next unblock:

- Run the live operator proof in a credentialed environment, then evaluate the
  receipts through the Gate decision contract.

### #4771 M13 provider peers and non-Codex live run

Status: open.

Blockers:

- The ToS review is complete and API-key BYOK connect paths for Anthropic and
  Gemini are built.
- The account-pool dashboard can now show Anthropic/Gemini accounts.
- The remaining acceptance leg requires one real Anthropic or Gemini
  API-key-backed account to be leased and consumed by a real Autopilot run.
- No obvious live Anthropic/Gemini key file or retained live non-Codex run
  fixture was available in the Scope lane.

Next unblock:

- Provide or connect a live Anthropic or Gemini API key, run a real non-Codex
  Autopilot execution through the lease path, and link the run evidence here.

### #4772 M14 MVP exit review and door-open decision

Status: open.

Blockers:

- #4767 live two-account rotation remains open.
- #4768 overnight unattended proof remains open.
- #4771 non-Codex real-run evidence remains open.
- Market live paid-labor proofs remain open where they affect public claims.
- The final decision record must name exact dates, issue refs, commits, smokes,
  accepted deferrals, and remaining claim limits.
- The public-signup/door-open claim must not advance until the cited receipts
  exist or the decision explicitly narrows the claim boundary.

Next unblock:

- Resolve or explicitly carve out the live proof issues above, then publish the
  M14 decision record.

### #4777 P1 first live negotiated labor job

Status: open.

Blockers:

- The market-key publisher is implemented but the live run still needs the
  dedicated market signing secret configured and deployed.
- A ref-only work request must return a retrievable relay event instead of the
  default unconfigured/rejected path.
- An independent contributor Pylon must quote the job.
- The requester must accept one quote with escrow reserve evidence.
- The contributor must execute and deliver output-only refs.
- A validator must accept the result.
- Escrow release and settlement/payout evidence must be recorded before any
  live labor claim.

Next unblock:

- Configure `FORUM_WORK_REQUEST_MARKET_SECRET_KEY`, deploy, rerun the no-spend
  probe, then execute the runbook with an independent provider.

### #4781 P5 backlog faucet live market proof

Status: open.

Blockers:

- The backlog faucet adapter and contract tests are merged.
- The live acceptance still requires real backlog issues to be listed through
  the open market.
- At least one listed issue must be quoted and completed by a non-owner
  provider.
- Validator acceptance and settlement evidence must be attached.

Next unblock:

- After the P1 market key and provider path are live, list the selected backlog
  issues and run at least one through non-owner completion.

### #4782 P6 spare-capacity provider mode

Status: open.

Blockers:

- The default-off provider-mode gate is merged.
- Live GO ONLINE proof is still missing.
- A provider Pylon needs explicit owner consent, pricing, capability,
  settlement readiness, own-work preemption, and earnings-visibility refs.
- The same-day owner-job plus stranger paid-job proof is not recorded.
- Paid provider claims remain blocked until settlement and payout visibility
  receipts are dereferenceable.

Next unblock:

- Bring a real provider Pylon online under the gate and record both owner-work
  preemption and stranger paid-job settlement evidence.

### #4783 P7 Lane C fanout

Status: open.

Blockers:

- The opt-in public-tier Lane C fanout policy is merged.
- No live product order has fanned out through the labor market.
- The proof depends on mission/work-order unification, settlement bridge,
  market inventory, artifact authority, validator policy, customer opt-in,
  budget-cap compliance, and provider public-trust tier evidence.
- Paid fanout claims remain blocked until the P4/P5/P6/P9 evidence chain is
  present for the run being cited.

Next unblock:

- Use a public-tier product order with owned capacity dark/limited, customer
  opt-in, and a quote within cap; then attach the market assignment,
  validation, artifact, release, and settlement refs.

### #4786 Autopilot MVP parent epic

Status: open.

Blockers:

- The parent should not close while #4767, #4768, #4771, #4772, #4777, #4781,
  #4782, #4783, and #4749 remain open unless the parent records the exact
  accepted post-MVP/open tail.
- Live paid Forum order, live autonomic funded tick, live provider routing,
  and MVP exit proof remain Gate/operator evidence.

Recently unblocked:

- The zero-debt architecture guard no longer blocks the parent on
  `forum-work-request-live-publisher.ts`: Worker throw count is back within
  budget and raw `JSON.parse` plus raw time/id/random counts are at zero.

Next unblock:

- Close or explicitly defer the open tail with issue-specific proof boundaries,
  then publish the parent closeout comment.

## Closed Issues With Deferred Proof Notes

These issues are closed for their scoped implementation but still feed the open
proof gates above.

| Issue               | Closed scope                                     | Deferred blocker carried forward                                                          |
| ------------------- | ------------------------------------------------ | ----------------------------------------------------------------------------------------- |
| #4766 M8            | Provider account pool dashboard                  | #4767 and #4771 still need credentialed live legs.                                        |
| #4769 M11           | Repo connect, data scope, placement explanations | Broader #4772/#4786 proof remains Gate-owned.                                             |
| #4770 M12           | Team budgets and spend-to-evidence joins         | Live funded payment/settlement evidence remains operator/Gate-owned where claims cite it. |
| #4773 A1            | API parity contract                              | Final MVP proof still depends on live Gate evidence.                                      |
| #4774 A2            | Agent payment in both currencies                 | Live MDK/L402/card-funded movement remains a live evidence boundary where cited.          |
| #4775 A3            | Forum-to-coding work-order linkage               | Live paid Forum order remains operator/Gate evidence.                                     |
| #4776 A4            | Autonomic coding-thread proposal contract        | Live funded autonomic tick remains operator/Gate evidence.                                |
| #4778 P2            | Mission/work-order unification                   | Lane C live fanout still requires proof using the merged contract.                        |
| #4779 P3            | Writeback symmetry                               | Live real-repo PR draft/human merge proof remains dependent evidence where claimed.       |
| #4780 P4            | USD-credit-to-sats settlement bridge             | Live settlement receipts are still required for paid labor claims.                        |
| #4785 P9            | Settlement visibility law                        | Every cited payout rung still needs dereferenceable visibility receipts.                  |
| #4813 Pack A parent | Pack A implementation closeout                   | Live MVP proof remains in #4768/#4772.                                                    |
| #4814 PA1           | Task supervisor                                  | Feeds #4768 proof receipts.                                                               |
| #4815 PA2           | Schedule and continuation receipts               | Feeds #4768 proof receipts.                                                               |
| #4816 PA3           | Notification and attention coordinator           | Feeds #4768 proof receipts.                                                               |
| #4817 PA4           | Companion projection                             | Feeds #4768 dual-surface proof.                                                           |
| #4818 PA5           | Smoke receipt authority                          | Evidence-only scaffold; live receipts still required.                                     |
| #4819 PA6           | Artifact and receipt ledger                      | Feeds #4768/#4772 proof trail.                                                            |
| #4820 PA7           | Structured event replay                          | Feeds #4768/#4772 proof trail.                                                            |
| #4821 PA8           | Usage budget and cost-stop projections           | Feeds M9/M10/M14 budget and smart-routing claims.                                         |
| #4822 PA9           | Permission/approval contract                     | Feeds unattended/headless proof boundaries.                                               |
| #4823 PA10          | Accessibility/non-interactive contract           | Feeds agent/Pylon non-interactive proof boundaries.                                       |

## Claim Boundaries

- Closed implementation issues are not enough to claim live MVP readiness.
- Contract tests and dry-run/default-off gates are not substitutes for live
  relay, provider, execution, validation, release, and settlement receipts.
- W3 (#4749) is a separate research/evaluation workstream and must not be used
  as Autopilot MVP door-open evidence.
- Any public copy about smart routing, non-Codex execution, unattended runs,
  labor-market settlement, or public signup must cite the specific live receipt
  refs named above.
