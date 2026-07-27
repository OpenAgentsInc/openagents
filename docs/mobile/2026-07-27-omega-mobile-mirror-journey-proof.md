# Omega mobile mirror journey proof (TM-06)

- Date: 2026-07-27
- OpenAgents issue:
  [OpenAgentsInc/openagents#9262](https://github.com/OpenAgentsInc/openagents/issues/9262)
- Parent:
  [OpenAgentsInc/omega#120](https://github.com/OpenAgentsInc/omega/issues/120)
- Receipt schema: `openagents.omega.mobile_mirror_journey_receipt.v1`
- Simulator receipt:
  `docs/mobile/evidence/2026-07-27-omega-mobile-mirror-simulator.json`
- Automated journey:
  `apps/openagents-mobile/tests/omega-mobile-mirror-journey.test.ts`
- Status: M0 through M2 pass against the mobile simulator harness. Live-host,
  shared-revocation, and physical-device evidence remain blocked or not run.

## Evidence boundary

This receipt records simulator truth only. It does not claim that a packaged
phone connected to a signed Omega install. It does not claim that the direct
host and relay enforce one live revocation record.

The receipt pins the exact OpenAgents and Omega commits observed by the run.
At the recorded Omega commit, `origin/main` contains no
`openagents.omega.device_bridge.v1` server, discovery V3 / QR issuer, or mirror
projection feed. Those host surfaces remain open in omega#121 through
omega#123. The receipt therefore says `blocked_live_host` even though all three
mobile simulator stages pass.

## Simulator journeys

| Stage | Automated journey | Result |
| --- | --- | --- |
| M0 | Start with no grant, connect from the exact QR bootstrap, admit a typed grant and snapshot, then observe the desktop and thread list through the already-subscribed zero-based home without a selection intent | `passed_simulator` |
| M1 | Start from a persisted generation/sequence cursor, fail the cached endpoint after a tailnet change, dial the signed announcement endpoint next, send the resume cursor and catch up to the returned snapshot | `passed_simulator` |
| M2 | Begin direct, observe relay evidence, receive an honest host shutdown, render relay plus exact staleness, reopen against a recovered engine snapshot and return to direct | `passed_simulator` |
| Revocation (mobile half) | Receive `grant_revoked`, clear the persisted grant and cursor, mark the phone unpaired and refuse grant reuse | automated, but the shared live direct/relay half is blocked |

## Commands

```sh
pnpm --dir apps/openagents-mobile run typecheck
pnpm exec vp test \
  apps/openagents-mobile/tests/omega-mobile-mirror-journey.test.ts \
  apps/openagents-mobile/tests/omega-mobile-mirror-journey-receipt.test.ts
pnpm --dir apps/openagents-mobile run test
pnpm run check:fast
```

## Residual live proof

Run these only after omega#121 through omega#123 land:

1. Pair the simulator from the QR issued by a running signed Omega host.
2. Confirm the home projects the host thread list without a tap.
3. Kill and reopen the app, and record the resumed generation and sequence.
4. Change tailnet reachability, and record cached-endpoint failure followed by
   announcement or manual recovery.
5. Restart `omega-effectd`, and record direct loss, relay or offline state,
   staleness, and recovery to direct.
6. Revoke the device grant once, and record refusal by both the direct bridge
   and relay lane.

When the owner's phone is available, repeat the admitted rows under the
omega#49 physical-device protocol. A simulator receipt must never be renamed or
promoted into physical-device evidence.

## Falsifiers

The evidence is invalid if any of these becomes true:

1. A simulator row is described as a packaged or physical-device pass.
2. A live-host or dual-transport revocation row is green while omega#121
   through omega#123 remain absent.
3. The receipt contains a pairing secret, private key, seed, mnemonic, `nsec`,
   or a Keychain path.
4. The M1 request omits the persisted cursor or skips the cached,
   announcement, QR, manual dial order.
5. The M2 header says direct without current direct transport evidence.
