# Room-worker optimization profiles

The ordinary builder keeps Binaryen `-O2` on Mac and uses `-O3` on Linux.
Both platforms keep checked Coax arithmetic, strict floating-point flags,
the river-specific C optimization, and the unchanged 1,060,000-byte budget.

The source closure initially matches all 290 inputs in the earlier
[qualified application audit](../coax-checked-state-2026-11-06/final-rebase-input-audit.json).
Before the named-target fixture changes, the Mac `-O2` build produces
1,053,804 bytes with SHA-256
`1189ca302eef79e6bca10b0dbb3280944b0747043bf8cc35d55cff4e0df5974c`.
This matches the earlier qualified worker exactly. Its unchanged passing
486/530/491-microsecond results remain in the original record and are not
rerun as confirmation.

The Mac `-O3` experiment measures 1,033,993 bytes, but its full-sample p95 is
515/611/650 microseconds at normal process priority. The last two profiles
fail the unchanged 600-microsecond limit. The smaller artifact alone does
not qualify that optimization on Mac.

The original Linux `-O2` artifact is reproduced at source `96b81d12ac` with
the admitted original Linux compiler and supported LLVM/Binaryen toolchain.
It measures 1,062,884 bytes and fails the canonical `everglade-room` budget.
The arbitrary retention filename also reports that it has no budget line;
that warning supplies no passing result. The separate canonical budget
command retains the actual refusal:

```sh
coax/web/tool.sh wasm-size budget apps/openagents-web/wasm-budgets.txt \
  everglade-room artifact/oversized-original.wasm
```

The original and final C/header banks, compiler identities, oversized
artifact, build logs, and final replay observations are retained with the
complete qualification record. Product source observations do not replace
the original compiler provenance receipts.
