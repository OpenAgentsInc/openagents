# Lev on the mesh

**Status:** proposed, and downstream of everything else. Nothing here starts
before Lev answers `/v1/systemone` on one machine with a fitted calibration
map. This page assumes `docs/kev/mesh-plan.md` and only describes where a
Lev worker differs from a kev worker, because the phases, the contract, and
the refusal posture are otherwise the same.

## Why it is worth the trouble

Every Apple Silicon Mac in the fleet is already a Lev worker. There is no
artifact to download, no cache to place, no license boundary to carry, and
no residency cost beyond what the operating system already pays. A kev-4b
worker needs 9.6 GB resident and a verified bundle before it offers a row.
A Lev worker needs a Mac that is switched on with Apple Intelligence
enabled.

That is the largest possible worker population at the lowest possible
onboarding cost, for the cheapest useful work the mesh could carry. If the
judgment quality measures out, it is the obvious first decision row.

## What gets harder

### There is no artifact to verify

Kev's admission story is digests: the bundle manifest pins each file, the
supervisor verifies every sha256 before offering the row, and `weight_hash`
means something. Lev has no base artifact. The strongest anchors available
are the base model signature the runtime reports, the operating system
build, and the digest of an attached `.fmadapter` package if there is one.

So a Lev catalog row cannot carry a `weight_hash` for the base, and a Lev
worker cannot prove which weights it ran. **The verification floor has to
move from digests to behavior.** That is a real weakening, and the honest
framing is that it buys the worker population above.

What replaces the digest:

- **Decision probes with pinned distributions.** A probe names a typed
  question set over a pinned state and the expected distribution within the
  row's tolerance. Kev's mesh plan already proposes this as a stronger check
  than a string match; for Lev it is the only check.
- **The isolation probe as a recurring floor check**, not only an admission
  check. A worker that reuses one session across questions is running a
  broken implementation, and the probe detects it.
- **Signature and record matching.** The worker reports its base signature,
  OS build, and the calibration records for the families it offers. A
  mismatch against the row is a refusal, not a warning.

### Replication says more, and means less

Because sampling is seeded, two Lev workers on the same base signature
running the same seeds should agree closely, and under greedy decoding they
should agree exactly if greedy is deterministic at all — which is a step 1
measurement, not an assumption. Divergence is therefore a sharper signal
than kev's numeric tolerance band: it points at a different base, a
different implementation, or a different OS build rather than at floating
point.

The catch is that the fleet will not be on one OS build. Cross-build
divergence is expected and not evidence of a bad worker, so replica
comparison has to be scoped to workers reporting the same signature. A
coordinator that compares across signatures is measuring Apple's release
schedule.

### Fan-out gets cheaper, relatively

Kev's mesh plan notes that splitting a request's questions across workers
multiplies state re-encoding, because a single worker would have packed
them. Lev never packs: one session per question means a single worker
already pays the state cost per question. Splitting ten questions across
five workers costs the same total encoding as running them on one, and
finishes sooner.

So question-level fan-out is the natural dispatch for Lev rather than an
oversized-request escape hatch. The merge is the same: questions partition,
question ids are preserved, and the merged answer is what one worker would
have returned.

### Metering has nothing to meter

Apple bills no tokens. The runtime's own token accounting was fiction in the
first bridge this workspace built, and honest usage truth took a dedicated
commit to fix; see [`apple-fm-surface.md`](apple-fm-surface.md). A Lev
receipt reports what the runtime actually provides and reports nothing where
it does not.

The unit that means something is the call: questions answered, samples drawn
per question, and wall-clock time on the worker's hardware. Whether any of
that is *priced* is a business decision about selling access to a free
resource on somebody else's machine, and it is downstream of the licensing
read below.

## The catalog row

A decision row for Lev carries different fields than a kev row:

```toml
[[decision_model]]
id = "lev-base"
family = "lev"
runtime = "apple-foundation-models"   # not a bundle; no artifact_bytes
base_signature = "…"                  # 40 hex, reported by the runtime
min_os_build = "…"                    # the build the row was measured on
adapter = ""                          # optional .fmadapter digest
backends = ["apple-neural"]           # the only one there is
question_types = ["noul", "choice", "score"]
max_options = 255                     # contract bound; schema bound measured separately
estimator = "l3"                      # l1, l2, or l3
calibrated_families = ["…"]           # records that must be present to admit
```

A row admits only when the worker's availability is available, its signature
matches, and every listed calibration record is present and current. A
worker offering `lev-base` without a calibration record for a requested
family refuses with `uncalibrated`, the same way an unverified kev worker
refuses rather than silently serving.

## Open questions

- **Licensing: resolved yes** by the owner on 2026-09-19. Serving Apple's
  on-device model through the mesh is admitted, and the gating issue is
  closed. What a row may claim is now a measurement question rather than a
  legal one.
- **What a Lev row is worth.** Pricing free inference on a contributor's
  own hardware is a different question from pricing GPU time, and the
  answer affects whether contributors run it at all.
- **OS-build churn.** The fleet's signatures will spread across builds
  continuously. How many concurrent calibration records the catalog
  maintains, and what happens to a worker whose build has no record yet, is
  a policy decision rather than a measurement.
- **Guardrail-driven refusals in aggregate.** If a meaningful share of
  dispatched work comes back as a guardrail refusal, the row's effective
  throughput is not what its latency suggests. Measure it before promising
  capacity.
