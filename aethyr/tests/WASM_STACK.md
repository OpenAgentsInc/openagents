The independent linear-memory stack oracle reads the actual optimized Wasm,
not a rebuilt diagnostic executable. LLVM supplies instruction decoding; the
oracle checks that its instruction spans cover the exact binary bodies before
doing separate affine stack-pointer and callgraph mathematics.

Run the decoder directly, then the oracle:

```sh
/opt/homebrew/opt/llvm/bin/llvm-objdump -d ACTUAL.wasm > ACTUAL.disasm
python3 aethyr/tests/wasm_stack_oracle.py --wasm ACTUAL.wasm \
  --disasm ACTUAL.disasm --usage DIAGNOSTIC_OBJECTS/*.su --grid \
  --receipt stack.json
```

The `.su` files come from direct C-only Clang diagnostic compiles of the
already admitted C/H, using the original optimization and floating-point flags
plus `-fstack-usage`. Do not reemit Coax or rebuild a passing product to obtain
them. Their static allocations complement the proof over the final optimized
binary; their frame sizes are not substituted for optimized function bodies.

The oracle follows structured branches and loops with entry-relative affine
SP values. Every reachable write must remain within the sum of all constant
decrement sites, and every normal return must restore entry SP. A leaking loop
cannot converge to that finite bound. Every indirect call includes every
non-null target in the fixed internal table. Unknown effects, dynamic SP
writes, dynamic C stack allocations, table mutation/import/export and
positive-allocation recursive components refuse. Zero-allocation recursive
components contribute no linear-memory allocation; this does not bound the
Wasm engine's separate execution stack.

The bound applies to a normal-entry invocation from initialized SP, followed
by restored normal returns. It does not authorize resuming after an arbitrary
trap or prove that arbitrary pointer writes are safe. Browser imports must not
reenter positive-stack functions synchronously. The Grid's storage-capacity
callback is explicitly checked as a zero-stack constant getter. Its frame
trap handler clears the engine; other trap recovery needs its own qualification.

The `--grid` check also derives arena arithmetic from the binary's constant
exports and immutable heap base. It requires the existing 4 MiB stack,
64 MiB memory ceiling, 400,000 vertices, 12 vertex floats and 112 uniform
floats, and checks the replay/journey exports are included.

The Coax harness constructs independent small Wasm inputs, uses the same
decoder, runs the oracle and checks receipts or refusals. It retains every
input, disassembly and refusal under `--out`:

```sh
COAX=/qualified/coax coax/web/tool.sh aethyr/tests/wasm-stack-checks.coax \
  --out /tmp/stack-controls
```

The controls were declared first in `wasm-stack-controls.json`: balanced
frame/loop cases each bound 16 bytes; seven losses cover loop growth,
unknown SP reads/writes, unrestored returns, positive-stack recursion,
an imported mutable table and corrupted disassembly bytes.

The final Grid qualification and exact artifact identities are recorded in
`docs/verification/aethyr-grid-final-stack-2026-11-02.json`. It replaces the
older 170-export stack calculation for that artifact; no browser, raster or
model gate was repeated for this audit.
