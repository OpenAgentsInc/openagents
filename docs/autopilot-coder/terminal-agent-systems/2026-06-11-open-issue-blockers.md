# Open Issue Blockers

Date: 2026-06-11

Updated: 2026-06-12

Source set: current open tail #4749, #4768, #4772, #4777, #4781-#4783, and
#4786, plus recently closed evidence and hygiene issues #4767, #4771, and
#4836-#4837, as delegated by
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

### #4772 M14 MVP exit review and door-open decision

Status: open.

Blockers:

- #4767 live two-account rotation is closed, but does not satisfy the separate
  overnight, market, or MVP exit gates.
- #4768 overnight unattended proof remains open.
- #4771 non-Codex real-run evidence is closed and no longer blocks M14, but it
  does not substitute for #4768 or market proof.
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

- The market-key publisher is implemented, the dedicated market signing secret
  is configured, and Worker version
  `f87df619-8678-40ad-872d-5ae35e953a80` is deployed.
- A ref-only no-spend work request returned `201` with a retrievable relay
  event instead of the default unconfigured/rejected path:
  - work request id: `f3da4627-246c-444d-885a-0f779964a779`
  - relay ref: `relay.public.market.0a2b94b3a5372b3a5cf8cbeb1325da9b`
  - job event id:
    `d480e175984bb3afafa92162438c9b56a1399b5631f9f88110fea11673520327`
  - evidence:
    `docs/labor/2026-06-12-p1-market-key-live-publisher-probe.md`
- That work request is no longer active: #4773 closed, so the row was expired
  with receipt ref
  `receipt.backlog_faucet.github_issue_closed.openagents.4773.20260612`.
- The live order book is currently empty. P1 needs a fresh currently-open
  bounded backlog target before the negotiated labor run can proceed.
- An independent contributor Pylon must quote the job.
- The requester must accept one quote with escrow reserve evidence.
- The contributor must execute and deliver output-only refs.
- A validator must accept the result.
- Escrow release and settlement/payout evidence must be recorded before any
  live labor claim.

Next unblock:

- Execute the runbook with an independent provider. The posted work request was
  expired after its backing issue closed, so select a new open target before
  publication.

### #4781 P5 backlog faucet live market proof

Status: open.

Blockers:

- The backlog faucet adapter and contract tests are merged.
- The prior live backlog work request
  `f3da4627-246c-444d-885a-0f779964a779` was relay-backed but is now expired
  because its backing issue #4773 closed.
- `/api/forum/work-requests` now correctly returns an empty order book with
  `generatedAt`, `maxStalenessSeconds: 0`, and the shared staleness contract.
- The live acceptance still requires real backlog issues to be listed through
  the open market.
- At least one listed issue must be quoted and completed by a non-owner
  provider.
- Validator acceptance and settlement evidence must be attached.

Next unblock:

- Select fresh currently-open backlog issues, list them through the live
  market, and run at least one through non-owner completion.

### #4782 P6 spare-capacity provider mode

Status: open.

Blockers:

- The default-off provider-mode gate is merged.
- The market-key publisher is configured. There is currently no open
  relay-backed work request after the #4773 row expired, so provider proof
  needs a new active listing.
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
- The labor-market bridge now publishes retrievable relay-backed work requests,
  but no live market quote/completion evidence exists yet.
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

- The parent should not close while #4768, #4772, #4777, #4781, #4782,
  #4783, and #4749 remain open unless the parent records the exact
  accepted post-MVP/open tail.
- Live paid Forum order, live autonomic funded tick, live provider routing,
  and MVP exit proof remain Gate/operator evidence.

Recently unblocked:

- The zero-debt architecture guard no longer blocks the parent on
  `forum-work-request-live-publisher.ts`: Worker throw count is back within
  budget and raw `JSON.parse` plus raw time/id/random counts are at zero.
- The P1/P5 market-key signing blocker is cleared: the Worker secret is
  configured, Worker version `f87df619-8678-40ad-872d-5ae35e953a80` is live,
  and a no-spend work request produced a retrievable kind-5934 relay event
  before it was later expired because the backing issue closed.
- Public hygiene is cleared for the current order book: #4836/#4837 are
  closed, closed GitHub issues are rejected before listing, and the live
  work-request list now declares freshness.

Next unblock:

- Close or explicitly defer the open tail with issue-specific proof boundaries,
  then publish the parent closeout comment.

## Closed Issues With Deferred Proof Notes

These issues are closed for their scoped implementation but still feed the open
proof gates above.

| Issue               | Closed scope                                     | Deferred blocker carried forward                                                          |
| ------------------- | ------------------------------------------------ | ----------------------------------------------------------------------------------------- |
| #4767 M9            | Live rate-limit rotation proof                   | Does not satisfy #4768, #4772, market proof, settlement, or parent MVP readiness.         |
| #4766 M8            | Provider account pool dashboard                  | Provider live legs now live in closed #4767/#4771 records; #4768/#4772 remain separate.   |
| #4771 M13           | Gemini provider-peer live run                    | Does not satisfy #4768, #4772, market proof, settlement, or parent MVP readiness.         |
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
| #4836 hygiene       | Product-promises freshness and announcement gate | Public claims still require exact evidence refs and #4772 decision limits.                |
| #4837 hygiene       | Forum work-request closed-issue/order-book guard | Market proof still requires fresh open listings and live provider/settlement receipts.    |

## Claim Boundaries

- Closed implementation issues are not enough to claim live MVP readiness.
- Contract tests and dry-run/default-off gates are not substitutes for live
  relay, provider, execution, validation, release, and settlement receipts.
- W3 (#4749) is a separate research/evaluation workstream and must not be used
  as Autopilot MVP door-open evidence.
- Any public copy about smart routing, non-Codex execution, unattended runs,
  labor-market settlement, or public signup must cite the specific live receipt
  refs named above.
