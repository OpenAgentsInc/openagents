# Actual browser bank and packet admission

Real `host.js` now uses the shared bank/span authorities through an
18,883-byte service reactor (32,000-byte app budget). The recording producer
supplies actual physical metadata, WAV and packet allocations in six modes.
All 54 scalar observations pass. The check-first observer kills 62/62 root
mutants without equivalents. Its first mutation failure was corrected with
an independent workload fixture; that failed log is retained.

Normal metadata admits both 60-byte WAVs. Metadata longer than its physical
capacity admits no view or assets. A first WAV beyond its allocation
refuses while the later valid WAV remains available, with aggregate failure.
Oversized, null and wrapping packets each refuse before a byte view. Every
mode records one length read, one positive drain and zero pending bytes
after the actual message handoff.

The old adapter's red receipt is retained: it copied the out-of-allocation
one-byte first WAV, did not report metadata-capacity refusal, and left each
13-byte packet pending because draining occurred only during drawing.
Version 1 now drains at the producer handoff. Pre-contract frame behavior
keeps its existing path.

The muted product regression passes gesture activation, canonical cue,
pause/held clocks, resume and engine replacement. Its first pre-gesture
observation missed the existing deadline; that log is retained. A later
run with improved failure diagnostics passed the same expectations and
time bound. No cause is inferred from the missing first observation; no
gate or DSP tolerance changed. Unchanged bank/sample and native memory
authority gates are reused.

The fixture build initially exposed duplicate Wasm export names in its
native-only function override. That failure is retained. Separate base
initializer/message export macros now leave those Wasm names on their
recording wrappers alone; native function behavior is unchanged.

Durable artifacts are under
`~/.openagents/aethyrforge/host-contract-12869/browser-spans/`: old/new host
sources, actual Wasm/cache C/headers/objects, source observations, original
receipt checks, logs and receipts. The original admitted 598 CPU compiler
emitted these pure product inputs from the genuine c224 root, with explicit
overlays. Its unchanged source digest is
`386db301954b87b24d07ea2d175643563eb437c613467d3adfd70f249071b598`;
the original receipt verifies before and after. Generated service/fixture
tables join 12/15 source files with zero differences. This product/source
join is not a new compiler release or provenance receipt.

Committed records in `browser-spans/` identify actual Wasm/host hashes.
Global browser PCM retention, promise/socket pressure, Linux opening-context,
HTTP/clip and ordinary world integration remain separate exits. Graphics
admission/defaults are unchanged, and both host issues stay open.
The browser fixture tools type-check with their new reactor staging/route
arguments. App build syntax, budget coverage and the measured reactor
budget pass; tools-check reports 1,788 tracked files and zero new files
without a firm reason.
