The native GPU harness is moving to Coax in two explicit epochs. The retained
epoch qualifies the two original historical shader profiles. Fresh shader,
fault-source and bridge emission orchestration remains pending; the original
Python/shell drivers and C references remain unchanged until that branch has
parity. This does not complete chapter 11 or qualify a current production realm.

`check-render-native-gpu.coax` owns source admission, all 18 sampling artifact
joins, bounded SSH/SCP calls, compiler observations and cleanup. Its worker owns
all authored uploads, pass descriptors, feature admission, expected values,
finite checks and verdicts. `native-gpu-effects.c` transports typed Vulkan SDK
effects through the unchanged historical `vks`/`vkc` interfaces. No shader
arithmetic moved into C. All Vulkan execution is offscreen.

The qualified shader compilers are `30868e8c…` for sampling and `693c3f37…` for
helper publication and buffer views. The worker and host tools use the separately
qualified older compiler `1fac3750…`. That image is exploratory and does not
match the current compiler sources. Both identities are recorded separately.

From the repository root, use a new absolute work directory and receipt:

```sh
COAX=/qualified/coax coax/web/tool.sh aethyr/tests/check-render-native-gpu.coax \
  --profile sampling --host coderos-4080 \
  --work-dir /tmp/new-sampling-work --receipt /tmp/new-sampling-receipt.json \
  --emissions "$PWD/docs/verification/data/aethyr-ch11-2026-11-01/sampling-vulkan-308/artifacts" \
  --source-root "$PWD/docs/verification/data/aethyr-ch11-2026-11-01/sampling-vulkan-308/inputs" \
  --coax /qualified/coax --historical-emissions --explore-stale-compiler
```

For `--profile helper`, use `native-helper` as the emissions directory and
`native-helper/source` as the source root. These historical inputs are mandatory
for this epoch. Each remote call is bounded by 360 seconds and the controller
has an 810-second budget. A successful `mkdir` establishes ownership of a unique
`/tmp/aethyr-native-gpu-<24 hex digits>` directory; every later path reaches
cleanup. No remote scripts, shell decision loops or environment dumps are sent.

The retained real comparisons match all 292 raw u32 words: 192 sampling words
across normal and two distinct missing-demotion controls, and 100 helper/depth/
range words. Sampling exits remain 0/1/1, with wrong counts 0/16/16. Helper
feature refusals remain 4/5 before any pipeline, and all four view fields match
the legacy observer exactly. Both validation counters stay zero. The original
0.00001 comparison remains unchanged; no precision mask is applied.

`native-gpu-parity-policy.coax` contains the complete pure receipt admission.
`check-native-gpu-parity.coax` applies it to retained files without another GPU
run. Its `--fault` modes qualify wrong lanes, validation, shader identity,
views and feature refusal losses. `check-native-gpu-tooling.coax` also corrupts
each of the 18 admitted sampling artifacts in an owned copy; every case refuses
before remote creation. The three owned authority policies kill 597/597 root
mutants, with zero survivors.

The first executed controller wrote an unquoted compiler hash in its summary
JSON. The original bytes remain in the archive. The offline qualifier quotes
only that known historical compiler field, parses the result, checks the saved
executed source epoch and verifies the retained GPU observations under the final
policy. Worker and command receipts were valid JSON throughout.

The payload files have before/after remote SHA joins. The compiled native images
were hashed and downloaded before execution; no after-execution image hash was
captured in this epoch. Worker shader identities fence files before and after
reads, rather than hashing the exact uploaded read buffer. These are local
qualification records, not execution attestation or protection against a
concurrent file rewrite that is restored between observations.

The full record is
[`aethyr-native-gpu-port-2026-11-02.json`](../../docs/verification/aethyr-native-gpu-port-2026-11-02.json).
