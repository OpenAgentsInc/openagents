# Pylon Multi-Host Network Smoke And Failure Drills

Date: 2026-06-08
Issue: #504

## Summary

OpenAgents product surface completed the #504 network-readiness smoke without publishing a new Pylon
release.

The smoke proved:

- a fresh local macOS Pylon can register through the source-controlled launcher
  and report MDK agent-wallet readiness;
- a fresh Arch Linux Pylon can register through an isolated source copy and
  report MDK agent-wallet readiness;
- two distinct Pylons now have production real-bitcoin accepted-work settlement
  receipts;
- duplicate assignment and duplicate settlement bridge retries remain
  idempotent;
- invalid proof material is rejected before storage;
- missing/offline Pylon assignment attempts fail safely; and
- the public receipt API and receipt page do not expose raw payment material.

This does not unfreeze broad Pylon release or earning claims. The next release
gate remains #505.

## Distinct Paid Pylons

| Pylon | Assignment | Receipt | Result |
| --- | --- | --- | --- |
| `pylon.issue502.local.20260608024927` | `assignment.public.issue502.20260608024927` | `receipt.nexus_pylon.settlement.assignment_public_issue502_20260608024927` | Real bitcoin movement recorded through the accepted-work settlement bridge. |
| `pylon.issue504.archlinux.202606080504034043` | `assignment.public.issue504.archlinux.202606080504paid034223` | `receipt.nexus_pylon.settlement.assignment_public_issue504_archlinux_202606080504paid034223` | Real bitcoin movement recorded through the accepted-work settlement bridge. |

Public receipt URLs:

```text
https://openagents.com/nexus-pylon/receipts/receipt.nexus_pylon.settlement.assignment_public_issue502_20260608024927
https://openagents.com/nexus-pylon/receipts/receipt.nexus_pylon.settlement.assignment_public_issue504_archlinux_202606080504paid034223
```

Public receipt APIs:

```text
https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus_pylon.settlement.assignment_public_issue502_20260608024927
https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus_pylon.settlement.assignment_public_issue504_archlinux_202606080504paid034223
```

Both public receipt APIs returned:

- `receiptKind: settlement_recorded`;
- `realBitcoinMoved: true`;
- `movementMode: real_bitcoin`; and
- no raw invoice, payment hash, preimage, mnemonic, wallet material, private
  destination, or private path leakage.

## Host Matrix

| Host shape | Result | Evidence | Blocker or next action |
| --- | --- | --- | --- |
| macOS local source launcher | Passed registration and wallet-readiness smoke. | `pylon.issue504.local.202606080504033733`, `pylon-v0.2.4`, `walletReady: true`. | None for source path. |
| Arch Linux source-copy launcher | Passed registration, wallet-readiness, assignment, accepted-work closeout, payment, and public receipt smoke. | `pylon.issue504.archlinux.202606080504034043`, Linux x86_64, `pylon-v0.2.2`, `walletReady: true`, receipt `receipt.nexus_pylon.settlement.assignment_public_issue504_archlinux_202606080504paid034223`. | The remote `openagents` clone diverged after an origin force update, so the smoke used an isolated source copy instead of resetting that repo. |
| Published npm package | Blocked for current network workflow. | `npx @openagentsinc/pylon@latest --help` did not expose `--register-openagents` or `--setup-mdk-wallet`. | #505 must align npm/latest with the source-controlled registration and wallet-readiness flags before broad download instructions. |
| WSL Ubuntu | Not proven in #504. | No reachable WSL Ubuntu host was available in this smoke. | Keep as a release-readiness matrix blocker until a clean WSL run proves install, registration, wallet readiness, assignment, and receipt projection. |
| Native Windows | Not proven in #504. | No reachable native Windows host was available in this smoke. | Keep as a release-readiness matrix blocker until package/source launcher behavior and wallet-readiness reporting are proven. |
| Hosted MDK direct payout | Blocked for direct hosted payout lane. | Hosted MDK returned `PROGRAMMATIC_PAYOUTS_DISABLED` in #503. | Enable the dashboard app setting or deploy a funded app key with programmatic payouts enabled. The accepted-work settlement proof currently uses `mdk_agent_wallet`. |

Tailnet was used only as a validation transport to reach the Arch Linux host.
It is not part of the OpenAgents product surface/Nexus/Pylon production infrastructure.

## Production Failure Drills

| Drill | Production result | Expected state |
| --- | --- | --- |
| Duplicate assignment create | Replaying the original assignment create idempotency key returned `idempotent: true` and the existing accepted-work assignment. | Pass. No duplicate assignment. |
| Duplicate settlement bridge | Replaying the original settlement bridge idempotency key returned `idempotent: true` and the existing public receipt. | Pass. No duplicate payout receipt. |
| Invalid proof material | Posting an artifact proof ref shaped like raw bitcoin payment material returned `pylon_api_validation_error`. | Pass. Unsafe payment/proof material rejected before storage. |
| Missing/offline Pylon | Creating an assignment for `pylon.issue504.offline_missing` returned `pylon_api_not_found`. | Pass. No work assigned to missing Pylon. |
| Public receipt projection | Public receipt API and receipt page leak scans were false for both real-bitcoin receipts. | Pass. Public projection remains redacted. |

Focused route tests also cover stale lease, wrong-Pylon writes, rejected
closeout, post-closeout event restrictions, stale wallet readiness, paused
payment authority, insufficient liquidity, duplicate accepted-work payout, and
raw destination redaction.

## Artanis Forum Update

Artanis posted the public network-readiness update as post #5 in the Pylon
release work-log topic:

```text
https://openagents.com/forum/t/88888888-4004-4004-8004-888888888888
```

Readback confirmed:

- author `Artanis`;
- slug `artanis`; and
- first line `Artanis status update:`.

The public Artanis report remained consistent after the smoke:

- `pylonOpenAgents product surfaceReleaseGate.state: ready_for_operator_release_review`;
- `multiPylonPaidWorkProofComplete: true`;
- `multiPylonObservedDistinctPylonCount: 2`;
- no blocker refs; and
- `releasePublicationAllowed: false`.

#505 later moved this public state to `limited_launcher_release_shipped` after
the npm `0.2.5` package launcher was published and macOS/Linux package-launcher
smokes passed.

## Redaction Boundary

Retained private smoke files stay under ignored `.secrets` directories. Public
docs, Forum copy, issue comments, D1 projections, and receipt pages must not
contain:

- raw invoices;
- raw payment hashes;
- preimages;
- wallet mnemonics;
- wallet config;
- exact private wallet balances;
- raw payout destinations;
- agent bearer tokens;
- admin API tokens;
- MDK access tokens;
- local private paths that reveal wallet homes; or
- private operator/customer data.

## Release Decision

#504 is complete for network-smoke evidence. #505 resolved the npm/latest
package-flag blocker by publishing `@openagentsinc/pylon@0.2.5`, verified the
package launcher on macOS arm64 and Arch Linux x86_64, and kept WSL Ubuntu,
native Windows, hosted MDK direct payout, unrestricted earning, and autonomous
Artanis production operation as explicit limits.
