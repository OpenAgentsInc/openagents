# M14 And #4786 Closeout Decision

Date: 2026-06-14

Issue scope: #4772 and #4786.

## Decision

The #4786 MVP parent can close with an explicit open-tail boundary. The
approved-user/core-team dogfood proof chain has receipts for the B/M/A ladder
and the first live paid labor job. This record does **not** open public signup
or broaden public product promises. Public-door copy remains gated outside this
decision.

The remaining open work is accepted as non-MVP or post-MVP tail:

- #4749 W3 student-program evaluation remains a separate research/evaluation
  track, not an Autopilot MVP door-open dependency.
- #4781 P5 backlog faucet remains open for three real backlog listings and
  standing market inventory.
- #4782 P6 spare-capacity provider mode remains open for same-day owner-job plus
  stranger-job priority proof and wallet settlement visibility.
- #4783 P7 Lane C fanout remains open for a real product order that opts into
  public-tier market fulfillment.

## Closed Ladder State

| Block                       | Issues                            | Closeout basis                                                                                                |
| --------------------------- | --------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| Bootstrap                   | #4755, #4756, #4757, #4758        | Closed child issues.                                                                                          |
| MVP M1-M13                  | #4759-#4771, including #4768      | Closed child issues, with #4767 live rotation and #4768 live two-lane proof.                                  |
| Agent parity                | #4773, #4774, #4775, #4776        | Closed child issues plus live paid labor evidence from P1.                                                    |
| Post-MVP early receipt      | #4777                             | Live negotiated labor job settled; see `docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md`. |
| Other post-MVP closed rungs | #4778, #4779, #4780, #4784, #4785 | Closed child issues and recorded scope boundaries.                                                            |

## Evidence Refs

M9 live rate-limit rotation:

- `docs/autopilot-coder/2026-06-12-m9-live-rate-limit-rotation-gate-record.md`
- `evidence.live.rate_limit_rotation.1`
- `provider_account_failover_receipt_c5f60166e739403799b2291c076d9801`

M10 overnight unattended proof:

- `apps/pylon/docs/proofs/m10-live-2026-06-14/`
- `autopilot_work_order.1531e063-71e4-49aa-a378-5d8d7fdbb3b3`
- `autopilot_work_order.fa64ac58-901c-4a90-a125-03792decb300`
- `pylon.m10.archlinux.20260614`
- production Worker version `00a6354e-a2c0-40ee-a0b0-7994d7c5f125`

M13 provider-peer proof:

- `docs/autopilot-coder/2026-06-12-m13-live-gemini-provider-gate-record.md`
- production Worker version `a10d2d08-fd81-4f50-ba01-e06ee90822ed`

P1 live labor proof:

- `docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md`
- work request:
  `work_request.public.b74bb55c-849c-43a3-b8d9-9a741316b528`
- result event:
  `result.public.pylon.labor_market.32751b623cbf3e01071182f7bc52b642d944b345404524871ffe8f5c03e905dd`
- release receipt:
  `receipt.labor_escrow.release.b74bb55c-849c-43a3-b8d9-9a741316b528.quote.public.pylon.labor_market.b97f312478504e9df212e333`

## Gate Result

For #4772:

- Decision: close the M14 decision record as a scoped MVP dogfood closeout.
- Public signup: not opened by this record.
- Public claim boundary: no copy may depend on #4749, #4781, #4782, or #4783
  until those issues close on their own evidence.

For #4786:

- Decision: close the parent epic with the exact accepted open tail listed
  above.
- Parent closeout does not close #4749/#4781/#4782/#4783.
- Any later public launch, broad market inventory, spare-capacity earning, or
  Lane C copy must cite those issue closeouts separately.

## Verification Commands

Commands run during this closeout:

```text
bun --cwd apps/openagents.com/workers/api test src/forum-routes.test.ts src/labor-live-rehearsal.test.ts src/labor-escrow.test.ts
bun test
curl https://openagents.com/api/forum/work-requests/b74bb55c-849c-43a3-b8d9-9a741316b528
curl https://openagents.com/api/forum/work-requests?limit=20
```

Results:

- Worker focused tests: 3 files passed, 84 tests passed.
- Delivered job verifier: 1 file passed, 1 test passed, 8 assertions.
- Public status endpoint: `workRequest.state: settled`.
- Public order book endpoint: `count: 0`.
