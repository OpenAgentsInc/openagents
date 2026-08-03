# Coldcard generator and owned-fixture reproduction

OFR-015 implements an independent TypeScript reproduction of the pinned
Coldcard, libngu, and MicroPython generator semantics. The implementation does
not import or execute target source. The target revisions remain those in the
frozen Coldcard reproduction manifest.

## Reproduced behavior

The evaluator models uint32 overflow at every Yasmarang state update, libngu's
independent public initial state, the selected provider stream, XOR combination,
little-endian partial-word copies, the adjacent-provider-output health check,
and `mp_obj_get_int_truncated`'s 32-bit reseed behavior. Its uniform sampler
retains the target's mask and retry behavior, and the keypad trace applies the
target's reverse Fisher-Yates loop.

The frozen corpus contains vulnerable, 32-bit-reseeded, approved-provider, and
five semantic mutation classes: guard, provider, initialization, call trace,
and reseed truncation. Each vector binds initial state, output, full digest-only
call trace, keypad permutation, retained width, and admitted worker profile.

## Where the expected values come from

This is the part that decides whether the corpus means anything.

Until 2026-08-03 every vector's expected digests were produced by the same
TypeScript reproduction the vector tests. Passing them proved that our
generator had not drifted from itself. It could not prove that our generator
matches Coldcard's, because nothing outside our code was ever consulted, and
`admitColdcardGeneratorEvidence` refused the whole corpus for exactly that
reason.

They now come from the target's own generator source. Both pinned Coldcard
firmware trees carry libngu at commit
`537519a829259622ea6b0334fbafd6cae852852f`, whose `ngu/random.c`
(sha256 `812585e47b2f9251693280c95b5e58558cbd564d62e4398b17388f9cb5198abb`)
defines `my_yasmarang`, `my_random_bytes`, `_bit_length`, and `_rand_below` —
the generator this reproduction models. That file is compiled **verbatim**
inside an admitted OpenAgents Cloud `live_gce` managed sandbox, from the
read-only tree baked into the guest image, and driven to produce every expected
value in the corpus.

The harness lives in `scripts/cloud/coldcard-generator-guest/`:

- Two translation units each `#include` the pinned `ngu/random.c` unmodified,
  giving two independent instances of the target's Yasmarang state. One is the
  libngu instance; the other stands in for the `CHIP_TRNG_32()` hook, which on
  the vulnerable firmware resolves to MicroPython's `ports/stm32/rng.c`
  fallback. That fallback is the same generator statement for statement,
  differing only in seeding, and cannot itself be compiled off the target
  because it dereferences STM32 peripheral registers.
- A small shim supplies the MicroPython declarations the target source compiles
  against. Every shim function aborts if reached, so none of them can
  contribute a value.
- Nothing transcribes the target's arithmetic. A test refuses any harness
  source that carries the transition's own shift constants.

Per vector the guest runs two passes and refuses to emit anything unless they
agree on output bytes, shuffle selections, and final generator state:

- the **target pass** calls the pinned `my_random_bytes` and `_rand_below`
  directly — the target's own health check, XOR order, memcpy, mask, and retry
  loop;
- the **mirror pass** steps the same compiled `my_yasmarang` and `_bit_length`
  one call at a time, so intermediate state is observable.

The mirror's stepping order is this reproduction's model of the target's
control flow. Checking it against the target's own functions is what makes a
wrong model fail loudly instead of being frozen into a digest.

The host packages those observations into the exact objects the reproduction
hashes. Only encoding happens on the host: every number that could expose a
defect — output bytes, per-call outputs, state after every step, retries,
selections — came out of the compiled target source.

Result: all eight vectors' expected digests were reproduced **unchanged**. The
TypeScript reproduction already agreed with the target source; what changed is
that this is now checked rather than assumed.

What this still does not prove is that the shipped firmware binary behaves like
its own source. That is the artifact-witness rung (OFR-014) and a separate
claim ref.

## Entropy and work estimates

Work factors are derived rather than copied from prose. Each dimension names an
assumption and correlation group. The calculator takes the maximum dimension
inside a correlated group and multiplies across independent groups, preventing
timer and RTC ranges from being treated as independent without saying so. It
then divides the exact candidate count by a measured throughput with ceiling
division. Hardware class, firmware, assumptions, profile, throughput, candidate
count, and projected duration remain reviewable.

The throughput is now measured rather than asserted. The same guest, on the
admitted `profile.sbx.gce.e2-small.v1` worker, ran a candidate search through
the pinned generator and reported what it counted: 201,637,888 candidates in
20.000003915 s, so 10,081,892 candidates per second and 178,538 s for the whole
1.8e12 candidate space. The previously published 250,000 per second and
7,200,000 s were hand-written, and no measurement supported them in either
direction.

Read the rate with its work unit. One candidate seeds the pinned fallback
provider from one (unique id ^ SysTick, RTC->TR, RTC->SSR) triple and runs the
pinned generator to a 32-byte wallet entropy draw. Downstream BIP39 and BIP32
derivation, which a real key search also pays per candidate and which costs far
more than the generator stage, is **not** included. `projectedSecondsCeiling`
therefore bounds the generator stage of a search on this worker profile; total
attacker time on the same profile is longer, not shorter. Three runs of this
harness on the same profile measured 9.99M, 9.61M and 10.08M candidates per
second, so treat the figure as one observation on shared capacity rather than a
constant.

## Secret-safe fixture boundary

Reproduction accepts only `synthetic` or `owner_authorized` fixtures. It derives
BIP39 and BIP32 material in memory across BIP44, BIP49, and BIP84 fixture paths,
compares digests of the expected xpub/address material, clears mutable entropy
and seed byte arrays, and returns only a receipt. The receipt contains no
mnemonic, xprv, or address. `retainedSecretMaterial: false` is checked rather
than claimed: the receipt is serialized and refused if it contains the mnemonic,
any four consecutive mnemonic words, the entropy, the seed, or the master xprv.
`liveValueLookupAttempted: false` is a structural claim, checked structurally —
the module contains no network client, and there is no network or value-oracle
argument in the reproduction API.

Artifact selection, generator behavior, entropy/work factor, and owned-fixture
matching retain separate references. A match at one rung does not promote
another rung.

## Where things live

| Surface               | Path                                                            |
| --------------------- | --------------------------------------------------------------- |
| reproduction          | `packages/forensic-contract/src/coldcard-generator.ts`          |
| frozen corpus         | `fixtures/forensics/coldcard/generator-vectors.v1.json`         |
| live capture evidence | `fixtures/forensics/coldcard/generator-live-capture.v1.json.gz` |
| guest harness         | `scripts/cloud/coldcard-generator-guest/`                       |
| guest driver          | `scripts/cloud/coldcard-generator-driver.mjs`                   |
| host packaging        | `scripts/cloud/coldcard-generator-package.ts`                   |
| live run              | `scripts/cloud/coldcard-generator-live.ts`                      |
| corpus rebuild        | `scripts/cloud/coldcard-generator-fixture.ts`                   |

## Reproducing the run

The run is owner-gated and costs live cloud money, about 3,000 µUSD per run. It
needs an IAP tunnel to a managed-sandbox control node whose image carries the
Coldcard trees, and the same environment the OFR-014 artifact-witness run uses.

```sh
node --import tsx scripts/cloud/coldcard-generator-live.ts \
  --apply --min-nanos 20000000000 --batch 8192 \
  --out artifacts/coldcard-generator-capture.json
node --import tsx scripts/cloud/coldcard-generator-fixture.ts \
  --capture artifacts/coldcard-generator-capture.json
```

The rebuild refuses any capture whose provenance is not an admitted worker run,
and it never carries the previous corpus's expected digests forward.
