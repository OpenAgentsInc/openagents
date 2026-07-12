# AUDIO-6 registered voice actions receipt

- Issue: #8739
- Date: 2026-07-12
- Deployed service: `openagents-audio-staging`
- Ready revision tested: `openagents-audio-staging-00005-hkj`
- Google provider: Speech-to-Text V2 `chirp_3`, location `us`
- Authentication: Cloud Run IAM identity token plus a distinct short-lived,
  exact-identity AUDIO-1 grant
- Fixtures: locally synthesized non-sensitive English PCM; deleted after each
  run and never committed
- Transcript logging: disabled; receipts retain only selected action kind,
  byte count, latency, final count, and gap count

## Deployed results

The checked-in `apps/openagents-audio/scripts/live-smoke.ts` streamed bounded
LINEAR16 frames through the deployed WebSocket and real Google STT, then fed
the final only into the same central typed selector used by Desktop.

```json
{"schema":"openagents.audio.stt_smoke.v1","finalCount":1,"gapCount":0,"audioBytes":66560,"latencyMs":6153,"transcriptLogged":false,"selectedAction":"interrupt"}
{"schema":"openagents.audio.stt_smoke.v1","finalCount":1,"gapCount":0,"audioBytes":62124,"latencyMs":2207,"transcriptLogged":false,"selectedAction":"message"}
```

The built Electron fixture separately proved a final routed through the closed
Desktop registry to `workspace.home`, plus truthful mute and stop:

```json
{"ok":true,"truth":["Mic capturing","Audio sending","Not retained","Reply audio on"],"registeredFocus":true,"muted":true,"stopped":true}
```

Focused renderer tests prove that the message peer becomes the existing
queue-until-idle follow-up path while a turn is active, and that interrupt is
the existing exact active-turn Stop path. The replay ledger rejects duplicate
finals, lost-ACK replay, and stale generations before any peer intent runs.
