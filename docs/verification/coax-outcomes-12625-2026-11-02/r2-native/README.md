# R2 compiler and native evidence

This directory separates historical v3 observations from the final focused
compiler/package. The final image `710c4f57…` uses checker `11d2c2a9…` and one
coherent canonical C/header tuple. `record.json` binds both epochs. Remaining
Source authority mutation scopes are not all zero; the parent handoff keeps
GitHub #12625 open. This completed native lane is not overall issue closure.

The final image passed the 41 original fixtures, all 18 inspection/ABI/profile
checks, the 24-case file adapter in each of three native profiles and both real
generic/default-constructor smokes. The eighteenth profile check compiles the
actual emitted compiler implementation at O0, O2 and fatal ASan/UBSan and checks
the changed meter, raw tag, propagation and layout policies against literal
oracles. The complete native bridge passed in 147.58 seconds. All nine
canonical C/header pairs reproduced exactly, and input pins stayed stable.
The six seed files were refreshed explicitly; only checker.c differed from
the already checkpointed seed epoch. Public checker headers stayed identical.

`focused-qualification.json` describes the 20-file local development package,
including its binary and complete generated compiler tuple. It is installed
directly at this isolated checkout's `coax/bin/coax`. There is no normal
provenance record or checked-release-cache promotion. The final full compiler
measurement, recorded by the parent, is 1,036,416 of 1,048,576 charged steps:
12,160 spare, with the analysis budget unchanged.

`focused-sources.tar.xz` retains generated C/headers, `proto_runtime.inc` and
the focused manifest. `focused-sources-manifest.json` lists every member hash.
No executable, object or dSYM is committed. The qualified binary remains local
at `/tmp/coax-12625-native/focused-package-final-v4/coax`, with its full hash in
the manifest. Inspect `final-source-pins.json` against the checkout before
rebuilding. A reconstruction writes only a new temporary directory:

```shell
r2_package_dir=$(mktemp -d /tmp/coax-r2-focused.XXXXXXXX)
tar -xJf docs/verification/coax-outcomes-12625-2026-11-02/r2-native/focused-sources.tar.xz -C "$r2_package_dir"
cc -std=c11 -O1 -g -c "$r2_package_dir/proto_importer.c" -o "$r2_package_dir/proto_importer.o"
c++ -std=c++20 -O1 -g -Wall -Wextra -Wno-missing-field-initializers -I "$r2_package_dir" -o "$r2_package_dir/coax-focused" coax/src/main.cpp "$r2_package_dir/proto_importer.o"
```

That recipe does not write `coax/bin`, a normal provenance record or a release
cache. A rebuilt binary needs its own observations; the recorded binary hash
does not assert reproducibility across output paths, host tools or machines.

The v3 image (`67e753…`) used checker `82ab123…`, with one coherent generated
parser/checker/importer tuple. All 41 original fixtures passed: 28 exact
diagnostic refusals and 13 positives executed at O0, O2 and fatal ASan/UBSan.
The 17 inspection/ABI/profile checks passed. The real file adapter passed 24
OS/fault cases per native profile, both generated-header include orders and
the incompatible-layout refusals. The previously failed complete native bridge
passed against this tuple in 211.98 seconds: actual parser/checker/Home/Grid
inputs and the deep semantic E0381 refusal. All nine generated C/header pairs
reproduced exactly. These observations do not silently qualify a later model.

The first attempted whole-compiler emission used the older trusted H model and
refused E0381. Its failed observation remains here. Per-module generation,
a matching stage0, then the complete importer avoided mixing old installed
headers with new graph records. Canonical seed generation used the bootstrap's
relative source paths. The other ten seed files were byte-identical; only the
lexer/parser/checker C/header pairs changed.

The sum-policy lane then closed its actual mutation findings. Original stock
sum-type coverage was 504/590, followed by preserved 566/590 and 573/577
observations. The final exact module has **570/570 killed, zero survivors**,
with all 1,219 baseline checks retained. Its 25 external oracle groups cover
93 cases without adding their graphs to the production compiler input.
`sum-types-zero-inputs.json` binds the copied procedure bytes and every input.

The layout policy has **53/53 killed, zero survivors**. Six literal endpoint
cases preceded the arithmetic change. A positive power-of-two alignment at
least four already rounds the u32 tag, so the payload offset is the alignment.
Signed32 metadata masks reject negative/oversized facts; admitted intermediate
arithmetic fits int64, and one final aligned extent check rejects overflow.
No charge counter or quota was removed. The original 64/57/7 observation is
preserved separately from the final scope.

The scan meter retains every original charge position and the same 1,048,576
limit. A small tested charge function makes its before-charge endpoint visible
to checks. Exact root-cycle, 256/257-entry stack and exhausted ancestor-probe
oracles require a conservative result and E0381 on exhaustion. Separate total
predicates check the six raw syntax tags and propagation context/family/error
identity. Optional layout uses the already canonical zero-size/alignment-one
none facts, avoiding redundant operand rewriting. There are no equivalence
waivers or skipped validation guards.

This is changed-code qualification. Full compiler mutation inventory, the full
bootstrap gate sequence, published release/cache/provenance promotion and
unrelated device/model/browser gates were not run. A focused development
installation must carry its distinct `focused-qualification.json`; it must not
claim `coax-provenance-v1` or overwrite a checked release cache.
