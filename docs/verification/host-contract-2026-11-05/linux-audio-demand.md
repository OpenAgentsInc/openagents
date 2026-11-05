# Linux lazy audio demand and callback ownership

The ordinary Linux adapter registers the generated canonical WAV bank without
decoding unused synth assets. A requested name resolves through the shared
WAV decoder or the existing pinned Ogg/companion admission path. Every source,
coded PCM, scratch and retained sample allocation keeps its existing Coax
budget ticket. No limit, selected content or codec tolerance changes.

Normal and ASan/UBSan/LeakSanitizer runs replay the frozen actual 16-frame
native startup command stream. All 120 fetches, the ambient bed and room are
available: 122 assets retain 7,739,161 words within the unchanged 128-ticket,
8,388,608-word cap. These are command replays, not newly produced Linux
world frames. The eager compatibility control remains a failed observation:
45 fetches are unavailable. The adapter does not evict assets to hide it.

The same runs explicitly warm all 48 registered synth WAVs and compare all
2,042,394 canonical words byte-for-byte. A fresh lazy context then executes
the complete default cue/room/bed/stop timeline: four ranges, 262,144 samples,
zero differences beyond the unchanged absolute 2^-20 bound, no shifts,
masks or nonfinite samples. A retained 1,920,000-word stereo bed survives
context retirement; its final reference releases the owner to zero.

File reads and decoding execute on the command caller outside the mixer
lock. Asset operations have their own serialized owner. A held decoder
probe invokes the actual callback renderer on another thread: it completes
1,024 frames before the decoder is released. A cue offered before a gesture
still returns -2 after that gesture; the next cue succeeds. Retirement
during another held decode refuses its late installation and releases every
word. This is a threaded SDK probe, not an observation of a physical device
callback during a held decode. Its ten-word observation authority kills
43/43 root mutants without survivors or equivalents.

The changed ordinary realm dispatcher separately passes the unchanged
32-word product trace normally and instrumented with actual muted PipeWire
callbacks. It preserves pre-gesture/covered clocks, pause/resume, single
packet draining, hostile physical spans, continued recording-face frames
and terminal device/context close. The existing session/product authorities
and 93-mutant checker are unchanged. The fixture explicitly warms its two
canonical assets so the existing allocation and sample observations remain
the same. Device output planes contain no nonzero samples.

The product observation tool and PCM SDK now have one physical Coax source
graph. The prior separate-object link failed on duplicate S_hash/S_hex;
the new graph owns each export once. No linker exception or generated-C
editing is used. Failed link and early observation/emission attempts remain
in the durable directory.

The original Linux 0.50 image is
`~/.cache/coax-release/8012c3c9473f51f98665796608d9194348a67a97904e07f652a72d53b12fa529/coax`,
SHA256 `5cc82481bf6db529e919a404134b6d18b02c984f443809af7426767af869af9c`.
Its unchanged original helper/receipt verifies against genuine protected
compiler root `9b1abcd3545ebe70bbfb7c6ba53d33c619bd37e0` before/after the
separately recorded product overlay and after these executions. Actual
profile sources and compiled source tables join 42/42 demand, 39/39 product
and 14/14 recording-fixture sources with zero differences. This does not
create a current compiler release or qualify a current whole realm.

Full C/headers/profiles, SDK input archive, commands, executable hashes,
fixture bytes and raw outputs are retained on both qualification machines
under `~/.openagents/aethyrforge/host-contract-12869/linux-audio-demand/`.
Small immutable observations are in [records/linux-audio-demand](records/linux-audio-demand/).
The ordinary Vulkan window, resource packaging and room/service composition
remain separate acceptance gates; the desktop default remains web.

Reproduce using the admitted prepared root and explicit installed SDKs:

```sh
coax/web/tool.sh aethyr/host/emit-linux-audio-demand.coax ROOT COAX NEW_EMISSION
coax/web/tool.sh aethyr/host/build-linux-audio-opus.coax \
  ROOT NEW_EMISSION NEW_SDK OPUSFILE.so --demand [--sanitize]
NEW_SDK/check-linux-audio-demand WEB_ROOT WAV_AND_PCM_DIR \
  PREFETCH.json RENDER_PLAN.json RENDER_DIR NEW_REPORT
coax/web/tool.sh aethyr/host/emit-linux-audio-product.coax ROOT COAX NEW_PRODUCT
coax/web/tool.sh aethyr/host/build-linux-audio.coax ROOT NEW_PRODUCT \
  NEW_DEVICE_SDK PIPEWIRE_DEV PIPEWIRE_LIB --product --combined [--sanitize]
XDG_RUNTIME_DIR=/run/user/1000 NEW_DEVICE_SDK/check-linux-audio NEW_REPORT ROOT
```
