# AUDIO-2 Google STT staging receipt

- Issue: #8735
- Service: `openagents-audio-staging`
- Region/project: `us-central1` / `openagentsgemini`
- Ready revision: `openagents-audio-staging-00002-7x4`
- Provider: Google Speech-to-Text V2 streaming, `chirp_3`, `us`
- Authentication: Cloud Run IAM identity token plus a distinct short-lived,
  exact-identity application voice grant
- Fixture: locally synthesized, non-sensitive English PCM; not committed
- Result schema: `openagents.audio.stt_smoke.v1`
- Result: one final, zero sequence gaps, 89,260 audio bytes, 2,657 ms wall
  latency, transcript logging disabled
- Log scan: no fixture phrase, transcript field, owner/session ref, payload
  length, or audio-byte field in application text/JSON logs
- Container proof: Cloud Build succeeded from the committed Dockerfile; local
  Docker was unavailable and is not claimed as evidence

This receipt proves the current Bun/Effect gateway can load the supported
Google Node gRPC client in the packaged Cloud Run runtime. The ADR's fallback
condition for moving the gateway to Rust was not met. Rust remains limited to
the Desktop media helper; Google credentials and recognition authority remain
server-side.
