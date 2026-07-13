# AUDIO-8 deployed real-microphone receipt

- Date: 2026-07-12
- Revalidated: 2026-07-13
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
token plus the opaque application grant. The current private service is revision
`openagents-audio-staging-00013-mks`, running as the dedicated
`oa-audio-retention@openagentsgemini.iam.gserviceaccount.com` identity with
Speech, service-use, Cloud SQL, private-bucket object, and exact-secret access.
On 2026-07-13 both services reported `Ready`; each routed 100% of traffic to the
named revision. Authenticated `/health` on the private service and public
`/health` on the edge both returned their exact bounded healthy response.
The authenticated application-grant issuer was also Ready on monolith revision
`openagents-monolith-00117-kdg` with 100% traffic at audit time.

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

## Current-main revalidation after the sentence/UI fix

The issue was reopened after the earlier receipt because the normal owner path
exposed fragmented finalization and high-frequency whole-shell reconciliation.
The fixes landed in `43a76b849c` and `f39ce24a28`; the current audited base is
`b0e579b0bba6ffbaa716f7ec42885a482d3c7ca8`, with Desktop package version
`0.1.0-rc.5`. Revision `openagents-audio-staging-00013-mks` runs the matching
server change: Google Speech-to-Text `chirp_3` now uses `SHORT` endpointing.
Google streaming Text-to-Speech remains `google-chirp3-hd-streaming`, voice
`en-US-Chirp3-HD-Sulafat`.

Fresh ref-only live receipts on 2026-07-13 were:

```json
{"schema":"openagents.audio.stt_smoke.v1","finalCount":1,"gapCount":0,"ackCount":20,"retainedSequenceCount":20,"reconciliation":{"missingObjects":[],"orphanObjects":[],"uncoveredSequences":[]},"exportedObjects":20,"deletedSegments":20,"audioBytes":120320,"latencyMs":4796,"transcriptLogged":false}
{"schema":"openagents.audio.tts_live_smoke.v1","ok":true,"assistantText":true,"mediaFrames":13,"adapterRef":"google-chirp3-hd-streaming","voiceRef":"en-US-Chirp3-HD-Sulafat","charsIn":36,"synthTtfbMs":185,"totalMs":565,"bytesOut":145662,"chunksOut":13,"outcome":"completed","transcriptLogged":false}
{"schema":"openagents.audio.barge_live_smoke.v1","ok":true,"interruptAckMs":0,"speechRefBound":true,"outcomeRefObserved":true,"transcriptLogged":false}
{"schema":"openagents.audio.long_fault_smoke.v1","durationSeconds":60,"realMicrophone":true,"muteStoppedEgress":true,"packets":565,"acks":565,"reconciliation":{"missingObjects":[],"orphanObjects":[],"uncoveredSequences":[]},"exportedObjects":565,"deletedSegments":565,"transcriptLogged":false}
```

The long fault smoke used the OS-default physical microphone and the current
native helper built from the audited base; it did not inject PCM and did not
record or print transcript content. The bounded STT and barge smokes used a
generated non-owner speech fixture, so they validate the deployed provider and
protocol path but are not represented as owner speech or owner acceptance.
The evidence diff and emitted JSON were scanned for token/secret/credential
values, transcript fields, raw media, local home paths, and private endpoint
query material; none is retained here.

## Verification

- `bun run --cwd apps/openagents-audio typecheck`
- `bun run --cwd apps/openagents-audio test` — 27 pass, 0 fail after the
  zero-based live-barge script regression was added
- `cargo test -p oa-desktop-audio` — 11 pass, 0 fail
- focused current Desktop voice/boundary/build suite — 124 pass, 0 fail;
  Desktop typecheck and production build pass with the native helper staged
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

## Owner acceptance and remaining gates

The owner recorded review in the active voice-roadmap thread on 2026-07-12,
before the later fragmented-final and shell-flicker report, fixes, and reopen.
That earlier review remains valid provenance for its historical build, but it
does not establish acceptance of `43a76b849c`/`f39ce24a28` or current main.

AUDIO-8 therefore remains open. Completion still requires one owner-reviewed
recording/provenance ref from the normal `oa` launch showing a complete spoken
sentence appears once, barge-in stops active playback and admits the new
sentence once, and Details/sidebar interaction remains stable while listening.
No transcript or audio content belongs in the public receipt. The earlier RC5
#8706 artifact predates AUDIO-8 and is not cited as proof of this code;
producing a newer signed/notarized RC from current `main` remains ordinary
release-system work.
