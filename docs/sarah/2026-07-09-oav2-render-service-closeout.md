# OAV-2 Render Service Closeout

Date: 2026-07-09
Issue: OpenAgentsInc/openagents#8612
Epic: OpenAgentsInc/openagents#8610
Hydralisk host: `sarah-avatar-gpu-1` (`g2-standard-8`, 1x NVIDIA L4)

OAV-2 is complete. The owned Sarah avatar render service now runs the
Hydralisk state machine, MuseTalk renderer, and WebRTC egress on the target L4
host with empty MuseTalk blockers and a browser-faithful real-session simulator
pass.

## Commits

- `80383251250fbffd68aff09bab15e34cb0cea87e` (`hydralisk`): adapts the
  LiveTalking MuseTalk `feature2chunks` call for the installed API shape while
  preserving compatibility with the older signature.
- `78faf3c371e3c3173683b6a456836b8664d2176e` (`hydralisk`): keeps WebRTC
  cadence during MuseTalk inpaint with the explicit
  `HYDRALISK_AVATAR_MUSETALK_FRAME_STRIDE` control.
- `73991d76e5d753abbbd3f38715b00ba893f80004` (`hydralisk`): records the GPU
  smoke evidence and live-host dependency guardrail.

Hydralisk evidence lives at
`docs/evidence/2026-07-09-oav2-musetalk-gpu-smoke.md` in that repo.

## Live Capability State

The host capability endpoint reported `rendererBackends.requested=musetalk`
with `musetalkBlockers=[]` before the final simulator pass. Preprocessed Sarah
clip references are staged on the host under `~/avatar-data/clip{0,3,4,5,6,8}`
as symlinks to the MuseTalk avatar outputs.

Operational guardrail: after GPU dependencies are installed on the live avatar
host, use `uv pip install -e .` for Hydralisk source updates. Do not use
`uv sync` there unless the GPU dependency set is being intentionally rebuilt,
because it can remove manually installed CUDA/MuseTalk runtime packages.

## Verification

Local Hydralisk test suite:

```text
uv run --extra dev --extra avatar pytest tests/test_avatar_*.py
# 75 passed, 2 warnings
```

Final real-session simulator command:

```text
hydralisk-avatar-sim --base https://34.63.208.229.sslip.io --min-fps 18 --max-gap 1.0 --idle-seconds 10
```

Final simulator result:

| Phase | Seconds | Frames | FPS | Max inter-frame gap |
| --- | ---: | ---: | ---: | ---: |
| connect | 2.10 | 49 | 23.31 | 0.084s |
| utterance_1 | 10.43 | 250 | 23.96 | 0.746s |
| between_turns | 5.00 | 120 | 24.00 | 0.072s |
| utterance_2 | 10.44 | 251 | 24.03 | 0.170s |
| idle_10s | 10.00 | 240 | 24.00 | 0.068s |

Server receipt summary:

```json
{
  "totalFramesReceived": 910,
  "framesRendered": 914,
  "speakingFrames": 318,
  "utterances": 2,
  "state": "idle",
  "stopReason": null,
  "failures": [],
  "pass": true
}
```

The failure class that kept #8612 open is closed: the renderer no longer dies
on the MuseTalk chunk API mismatch, the service stays at realtime WebRTC
cadence during speaking phases, and the extended idle phase returns cleanly to
`idle` after a second turn.

## Remaining Work Moves To Later Lanes

- OAV-5 (#8615): pre-rendered opener/cache-hit takes on the owned renderer.
- SQ-3 (#8620): perfect opener library integration and defect repair.
- SQ-2 (#8619): complete the tightened experiment matrix.
- OAV-6 (#8616): quality ladder experiments beyond the baseline MuseTalk path.

None of those remain OAV-2 blockers.
