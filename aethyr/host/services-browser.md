# Browser service admission

Host contract v1 loads `host-services.wasm` before installing its producer.
The separate reactor calls the same Coax authorities without adding them to
the world's load graph. A missing reactor refuses installation. Pre-contract
pages keep their existing adapter.

`services-face.c` projects measured scalar arguments to `htb_copy_span` and
`hab_*`. Before JavaScript forms a metadata view, its complete physical
capacity, reported count and linear-memory extent pass the span authority.
The immutable WAV allocation passes separately. Each row then passes the
bounded reactor line buffer and bank registry before a WAV view or copy.
Invalid or duplicate rows produce aggregate failure while later valid rows
can still be admitted.

Packet length drains once after each producer handoff. Its actual capacity
and complete linear-memory extent pass before a byte view. Null, wrapping
and oversized spans refuse without another length poll or GPU frame wait.

`build-services.coax ROOT OUT.wasm` uses the existing Wasm toolchain and
compiler admission. The app includes `/build/host-services.wasm`. Prepared
browser core, lifecycle and audio fixtures include it beside `fixture.wasm`;
their tools copy it into the fixture's `build/` directory. Frame-origin
configurations supply that explicit route, or the frame-ownership tool
accepts `--services REACTOR.wasm` and records the immutable input.

`openPcmMemory(services)` wraps those existing PCM extent/reservation
primitives in concrete SDK allocation handles. `allocate(frames,channels)`
reserves before constructing an exact `Float32Array`; `copy` first requires
the complete measured sample count. `claim` reserves a plane before a
separate SDK effect allocates it. Allocation failure releases its ticket.
Each handle owns one reference, supports `retain`/`release`, and exposes a
sample view borrowed only while the handle is live. A caller retaining that
view must retain the handle. Final release ends use and clears the owned
view before returning its ticket. Retiring the memory owner refuses new
allocations while existing immutable references keep their exact budget.
This tracks application-owned references, not JavaScript garbage collector
or resident-memory timing.

Actual browser allocation/span observations pass the unchanged native
96-mutant authority and 11-mutant retention authority. Wiring these handles
through the AudioWorklet and decode pipeline remains separate. Global
browser PCM ownership, bounded command promises, socket queues and ordinary
Vulkan play remain open.
The actual six-mode bank/packet observer kills all 62 root mutants without
equivalents; its receipt does not establish complete host conformance.
