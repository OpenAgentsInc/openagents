# AUDIO-8 deployed real-microphone receipt

- Date: 2026-07-12
- Issue: [#8741](https://github.com/OpenAgentsInc/openagents/issues/8741)
- Epic: [#8733](https://github.com/OpenAgentsInc/openagents/issues/8733)
- Evidence rung: built and packaged real-Electron owner-dogfood candidate;
  signed distribution remains governed by [#8706](https://github.com/OpenAgentsInc/openagents/issues/8706)

## Deployed topology

The microphone client does not possess Google IAM credentials. It presents one
short-lived, identity-bound application grant to public Cloud Run edge revision
`openagents-audio-edge-staging-00001-frh`. That service runs as
`openagents-audio-edge@openagentsgemini.iam.gserviceaccount.com`, holds only
`run.invoker` on the IAM-private audio service, and forwards a Google identity
token plus the opaque application grant. The private service is revision
`openagents-audio-staging-00012-bbs`, running as the dedicated
`oa-audio-retention@openagentsgemini.iam.gserviceaccount.com` identity with
Speech, service-use, Cloud SQL, private-bucket object, and exact-secret access.

The Cloudflare bridge spike was removed after live probes returned the account
platform limit `1027/429`; `openagents.com` itself also currently resolves
through Google Frontend, so Cloudflare path routes were not authoritative. No
dead Cloudflare audio Worker remains deployed.

## Defects found by the deployed journey

The acceptance run found and corrected four defects that fixture-only evidence
had not exposed:

1. Desktop could not call an IAM-private Cloud Run WebSocket with only its
   application grant. The narrow public edge now injects the host-only Google
   identity without terminating application authority.
2. Rustls had two enabled crypto providers and panicked on the native worker
   thread before TLS. The helper now installs the Ring provider explicitly and
   uses bounded IPv4 connection attempts so an unreachable IPv6 address cannot
   hold capture indefinitely.
3. CoreAudio callbacks emitted roughly 5 ms packets, outrunning encrypted GCS
   plus SQL persistence. The helper now coalesces exactly 100 ms / 3,200-byte
   PCM packets before transport.
4. Rust's frozen generation began at sequence zero while the server initialized
   its watermark to zero, silently acknowledging the first frame without
   retention. The server now uses an internal `-1` watermark and proves zero-
   based conformance in its session and WebSocket tests.

An ACK is now issued only after the encrypted object and its SQL manifest both
commit. A retention failure cannot advance the client watermark. Accepted
frames are therefore either durable or explicitly unacknowledged/gapped.

## Machine receipts

Real OS-default microphone capture through the release Rust helper, with an
acoustic fixture played over the MacBook speakers (no PCM injection), then a
canonical assistant-message call through the public edge to Google Chirp 3 HD
and native playback, produced:

```json
{"schema":"openagents.audio.real_microphone_smoke.v1","devicePath":"os_default_input","injectedPcm":false,"live":true,"packetCount":56,"ackCount":56,"finalCount":1,"playbackCount":14,"canonicalSpeak":true,"transcriptLogged":false}
```

The 60-second real-microphone fault run muted capture for three seconds,
verified zero packet growth during mute, resumed the same session, drained all
ACKs, reconciled private GCS against manifests, exported, and deleted:

```json
{"schema":"openagents.audio.long_fault_smoke.v1","durationSeconds":60,"realMicrophone":true,"muteStoppedEgress":true,"packets":562,"acks":562,"reconciliation":{"missingObjects":[],"orphanObjects":[],"uncoveredSequences":[]},"exportedObjects":562,"deletedSegments":562,"transcriptLogged":false}
```

The deployed PCM/STT/storage journey separately returned exactly one final,
16/16 durable ACKs, no gaps, exact reconciliation, 16 exported objects, and 16
deleted segments. Prior AUDIO-7 evidence records real Chirp 3 HD synthesis at
198 ms first-byte latency and qualified barge-in acknowledgement p95 of 2 ms.

## Verification

- `bun run --cwd apps/openagents-audio typecheck`
- `bun run --cwd apps/openagents-audio test` — 26 pass, 0 fail
- `cargo test -p oa-desktop-audio` — 11 pass, 0 fail
- `bun run --cwd apps/openagents-desktop verify` — 1,072 pass, one unrelated
  documented H5 skip, build and real Electron smoke pass
- `bun run --cwd apps/openagents-desktop package:mac` — arm64 app and packaged
  `Contents/Resources/native/arm64/oa-desktop-audio` produced
- direct launch of the packaged `OpenAgents.app/Contents/MacOS/OpenAgents` in
  smoke mode — complete, including persistent voice HUD/action/barge fixture
- 60-second real microphone run — 562 packets, 562 durable ACKs, exact
  reconcile/export/delete

The packaged artifact is intentionally the issue's lower rung: its helper is
present and arm64, but the package has no valid distribution signature or
notarization and therefore does not satisfy #8706.

## Fault and privacy coverage

The combined AUDIO-1 through AUDIO-8 suites cover duplicate, out-of-order,
gap, replay, stale generation, bounded reconnect, provider timeout/quota,
storage outage, partial object write, digest collision, legal hold, TTS
underrun, stale playback, device change, suspend, revocation, parent exit, and
helper crash. The live run adds real TLS, CoreAudio, Google STT, private IAM,
GCS/SQL, mute/resume, export, and delete. Receipts and service logs contain
counts/refs only; no transcript or raw media is printed, placed in Sync, or
included in support output.

## Owner acceptance and remaining release rung

The owner recorded review in the active voice-roadmap thread on 2026-07-12:
“i reviewed it. continue. close if nothing else. proceed.” The canonical
assistant-message → closed `voice.speak` command → grant-gated `/v1/speak` →
Chirp/native playback seam was then added and live-proven before closeout.

AUDIO-8 and the implementation epic therefore close at the owner-accepted,
documented packaged real-Electron lower rung. The earlier RC5 #8706 artifact
predates AUDIO-8 and is not cited as proof of this code; producing a newer
signed/notarized RC from current `main` remains ordinary release-system work.
