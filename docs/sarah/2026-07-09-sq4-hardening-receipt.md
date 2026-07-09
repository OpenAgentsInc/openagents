# SQ-4 hardening receipt (#8621)

Date: 2026-07-09
Issue: OpenAgentsInc/openagents#8621
Epic: #8610

## Shipped (this + prior same-day landings)

| Item | Where | Evidence |
|---|---|---|
| aiortc answerer track pairing | hydralisk | `efb17d1` |
| Keepalive-vs-warmup race | hydralisk | `f291823` |
| Connected-peer liveness | hydralisk | `fd3a410` |
| Stale-slot eviction | hydralisk | `984c6c2` |
| Render-loop never dies silently + real-session sim gate | hydralisk | `d3a8fdb` |
| Sentence-streamed TTS + greeting + browser ASR | openagents | `8a61109328`, `155326587e` |
| Behavior contracts + e2e smoke on deploy | openagents | `04abde6646` |
| **MuseTalk placeholder-bbox fail-closed** | hydralisk | this land — identity frame on zero-size/invalid bbox |
| **Honest fps labeling** | hydralisk | `HYDRALISK_AVATAR_HONEST_FPS_LABEL` + capabilities `honestFps` |
| **LiveAvatar instant-fallback flag** | openagents | `SARAH_AVATAR_OWNED_FALLBACK` (default on when LiveAvatar armed); caps never fall through |

## Still deferred (not blocking SQ-4 close)

| Residual | Why deferred |
|---|---|
| Eyes-open speaking-window pinning | Needs per-clip face-window metadata pass over the catalog; next OAV quality matrix lane |
| Multi-session capacity >1 on one L4 | Hardware: one L4 stream today; eviction + busy UX already honest |
| Native owned-ASR (replace browser SpeechRecognition) | Separate product lane; browser ASR contracts green |
| TensorRT 24fps on L4 | Optional; honest 20fps via `HYDRALISK_AVATAR_FPS=20` |

## Operator flags

```bash
# Prefer owned; fall back to LiveAvatar when GPU/render service is busy (default when LIVEAVATAR_API_KEY set)
export SARAH_AVATAR_RENDERER=owned
# export SARAH_AVATAR_OWNED_FALLBACK=off   # fail hard instead

# Honest paced fps when L4 cannot hold 24
export HYDRALISK_AVATAR_FPS=20
export HYDRALISK_AVATAR_HONEST_FPS_LABEL=1
```
