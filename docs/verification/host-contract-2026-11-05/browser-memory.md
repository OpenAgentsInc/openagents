# Actual browser PCM allocation handles

The browser SDK now reserves a shared Coax PCM ticket before allocating or
copying a sample plane. Its actual headless component workload passes the
same `mamm_accept` and `harr_accept` authorities as Linux/Metal: 20 memory
fields, 50 bank fields, four retention fields and terminal-owner refusal.
Their unchanged 96/11 root mutation results are reused.

Negative, zero, overflowing and unsupported channel/frame extents refuse
before allocation. A real 8,388,608-word stereo plane exhausts the budget;
another sample refuses. All 128 one-word tickets fit, the 129th refuses,
release/reuse admits a complete stereo frame, and final release returns the
total to zero. A six-word copy preserves its last sample exactly. Actual
source bytes 37–80 in a producer `WebAssembly.Memory` exercise complete
bank views, duplicate/malformed/outside spans and the final exact byte.

A retained six-word allocation survives owner retirement and its first
reference release. The final release returns its ticket and clears its
owned view. Retired owners refuse new allocations. These are application
reference observations; no garbage collector or resident-memory timing is
inferred. Views are borrowed only while their allocation handle is held.

Artifacts are under
`~/.openagents/aethyrforge/host-contract-12869/browser-memory/`. The new
reactor adds only a measured bank-count projection to the previously
qualified pure Coax closure, whose C/cache and 12-source exact join remain
unchanged. The original admitted 598/c224 preparation remains separate from
product overlays. Exact reactor, effects source, observer executable and
actual receipt hashes are retained. A mistaken stale staging build was
isolated as `stale-services.wasm`; the actual gate uses the new artifact.

This component is not yet the AudioWorklet's global owner. Decode scratch,
transferred planes, pending installs and final node/context retirement must
use these handles before the browser PCM integration exit can pass. Existing
DSP samples/device and full-world/graphics/default gates retain their
separate scope. No audio device or GPU world was used by this workload.
