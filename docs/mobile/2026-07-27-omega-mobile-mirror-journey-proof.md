# Omega mobile mirror journey proof (TM-06)

- Date: 2026-07-27
- OpenAgents issue:
  [OpenAgentsInc/openagents#9262](https://github.com/OpenAgentsInc/openagents/issues/9262)
- Parent:
  [OpenAgentsInc/omega#120](https://github.com/OpenAgentsInc/omega/issues/120)
- Receipt schema: `openagents.omega.mobile_mirror_journey_receipt.v2`
- Simulator receipt:
  `docs/mobile/evidence/2026-07-27-omega-mobile-mirror-simulator.json`
- Automated journey:
  `apps/openagents-mobile/tests/omega-mobile-mirror-journey.test.ts`
- Status: M0 through M2 and revocation pass against the mobile simulator
  harness. The exact Omega host dependencies are on `main`. A signed-host run
  and a physical-device run were not done.

## Evidence boundary

This receipt records simulator truth only. It does not claim that a packaged
phone connected to a signed Omega install.

The receipt pins the exact OpenAgents and Omega commits observed by the run.
The recorded Omega commit contains the discovery V3 record, the QR issuer, the
authenticated `openagents.omega.device_bridge.v1` server, and the bounded
mirror feed. Omega tests cover host admission, resume, revocation, and engine
generation recovery. These host facts are dependency evidence. They are not a
record of a packaged phone connection.

## Simulator journeys

| Stage      | Automated journey                                                                                                                                                                                                                                  | Result             |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------ |
| M0         | Start with no grant, connect from the exact QR bootstrap, admit a typed grant and snapshot, then observe the desktop and thread list through the already-subscribed zero-based home without a selection intent                                     | `passed_simulator` |
| M1         | Start from a persisted cursor. Let the cached socket open and stay silent. At the admission deadline, close it and dial the signed announcement endpoint. Ignore a late cached frame. Send the resume cursor and catch up to the returned snapshot | `passed_simulator` |
| M2         | Begin direct, observe relay evidence, receive an honest host shutdown, render relay plus exact staleness, reopen against a recovered engine snapshot and return to direct                                                                          | `passed_simulator` |
| Revocation | Receive `grant_revoked`, clear the saved grant and cursor, mark the phone unpaired, and prevent the saved authority from entering direct or relay recovery                                                                                         | `passed_simulator` |

## Commands

```sh
pnpm --dir apps/openagents-mobile run typecheck
pnpm exec vp test \
  apps/openagents-mobile/tests/omega-mobile-mirror-journey.test.ts \
  apps/openagents-mobile/tests/omega-mobile-mirror-journey-receipt.test.ts
pnpm --dir apps/openagents-mobile run test
pnpm run check:fast
```

## Optional live proof

The host dependencies are on `main`. Use these steps for a later signed-host
receipt:

1. Pair the simulator from the QR issued by a running signed Omega host.
2. Confirm the home projects the host thread list without a tap.
3. Kill and reopen the app, and record the resumed generation and sequence.
4. Change tailnet reachability, and record cached-endpoint failure followed by
   announcement or manual recovery.
5. Restart `omega-effectd`, and record direct loss, relay or offline state,
   staleness, and recovery to direct.
6. Revoke the device grant once, and record refusal by the direct bridge and
   the relay recovery lane.

When the owner's phone is available, repeat the admitted rows under the
omega#49 physical-device protocol. A simulator receipt must never be renamed or
promoted into physical-device evidence.

## Falsifiers

The evidence is invalid if any of these becomes true:

1. A simulator row is described as a packaged or physical-device pass.
2. A signed-host or physical-device row is green without a run from that
   surface.
3. The receipt contains a pairing secret, private key, seed, mnemonic, `nsec`,
   or a Keychain path.
4. The M1 request omits the saved cursor, does not close a silent candidate at
   the admission deadline, accepts a late frame from an abandoned candidate,
   or skips the cached, announcement, QR, manual dial order.
5. The M2 header says direct without current direct transport evidence.
