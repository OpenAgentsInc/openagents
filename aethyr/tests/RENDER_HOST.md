# The production browser capture controller

Run the actual muted, headless DOM host with:

```sh
coax/web/tool.sh aethyr/tests/check-render-host.coax \
  PRODUCER.wasm PACKAGE_DIR PAGE.html NEW_EMPTY_OUTPUT_DIR \
  --start --half --texture-stream --require-draws \
  --size=320x180 --physical=320x180
```

The output directory must be new or empty. An existing artifact, including
an empty file, or an input page inside that directory is refused before
observation files are written. The controller uses the original three
background flags, always headless and muted, and sets its viewport through
CDP Emulation. It does not bring a window to the owner's screen.

The positional arguments and original flags remain available:

| Flag | Observation |
| --- | --- |
| `--start` | Warm the host, send the actual Enter press/release, then arm capture only after scene readiness and any requested media readiness. |
| `--half` | Request rgba16float capture and explicit clip steps. Without it, retain the host's normal capture format. |
| `--size=WIDTHxHEIGHT` | CSS viewport, default 320×180, each dimension 1–8192. |
| `--physical=WIDTHxHEIGHT` | Explicit capture-only ResizeObserver device-pixel-box fixture; the literal argument and actual observation are retained. |
| `--dpr=NUMBER` | JavaScript Number spellings, including decimal/exponent/base/whitespace forms; finite and in (0,4]. SDK refusal remains an actual failure. |
| `--texture-stream` | Service real texture-capture callbacks through Coax-owned byte transfers. |
| `--require-draws` | Require a captured scene draw. Background captures before Start can otherwise have zero draws. |
| `--pending-index`, `--pending-layer`, `--pending-pack` | Hold the actual selected resource effect, observe a real asset row, replace the engine, release it, and require retained identity/bytes/texture/binding ownership through another replacement. |
| `--reload-check` | Run JSON, digest, pipeline, success, digest, success trials in order; require original bank/identity/validation/submission decisions. |
| `--media-controls` | Stage a real upload, refuse a wrong media extent before any queue effects, preserve pending work, restore it, and observe recovery. |
| `--engine-reload`, `--engine-failure` | Observe parallel renderer replacements and an engine replacement, or a failed replacement which retires its binding. |
| `--media-fixture`, `--media-video` | Use the production media path; generated video is a silent 64×32 canvas stream. |
| `--media-reloads` | Preserve ready media through renderer and engine replacement, recapture, then retire the owner and refuse late allocation. |

Unknown flags remain ignored and the first equals-form size/physical/DPR
argument wins. `--surface-root DIR` selects an explicitly fenced fixture
surface for qualification. No implicit Start is added to another flag.
The pending guard only waits for an observed asset row; it never invents
readiness or bypasses an absent/incompatible producer.

Coax owns the schedule, deadlines, source identities, all admission and
lifecycle verdicts, artifact names, chunks, checksums and native frame
configuration. The narrow JavaScript file performs browser SDK effects and
retains actual object/byte handles. Each transfer uses the declared origin
sink in chunks of at most 64 MiB, with a 256 MiB per-artifact limit. Exact
counts, digest, ACK, stored bytes and source/origin finalization are joined.
Both streaming and non-streaming modes retain `gpu.bin`, uniforms, HUD,
draw/index data, all declared buffers/textures, media metadata and
`frame.config`.

The v2 receipt retains actual DOM/status facts, window errors/rejections,
raw CDP event count and precise compiler/tool/source identities. The current
CDP client does not retain raw event objects. `armResult` is the actual SDK
value instead of the old Runtime.evaluate envelope. Readback-callback and
DOM loading-progress timing can differ while a streaming callback is
serviced; their values and inequalities remain explicit. A background HUD
snapshot is not evidence of a fully ready scene or identical numerical
inputs. Require draws and declare the input/frame profile for such checks.

Qualification, independent legacy references, retained failures and the
complete source/body archive are recorded in
`docs/verification/aethyr-render-host-tool-port-2026-11-02.json`. The legacy
prototype pipeline hook was shadowed by an actual device's own method; the
qualified fixture hooks that actual method and requires an observed planted
rejection. Its original failure and the bounded asset-presence adaptation
are retained. The renderer and its policy are unchanged by these fixtures.
