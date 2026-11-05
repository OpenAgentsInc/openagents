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

The bridge also exposes the existing PCM reservation/extent primitives for
subsequent adapter work. Global browser PCM ownership, bounded command
promises, socket queues and ordinary Vulkan play remain separate exits.
The actual six-mode bank/packet observer kills all 62 root mutants without
equivalents; its receipt does not establish complete host conformance.
