# Is a compiled function just another plugin?

**Question asked:** ProgramAsWeights argues for small, ownable, callable units
of capability. Our own plugin system in `coder` argues for the same thing.
Are they the same design with different payloads, and can our socket take
weights?

**Answer:** the socket is the same shape and **will not take weights** — every
load-bearing mechanism in it depends on the payload being small, pure, and
deterministic, and an adapter is none of the three. But that is the less
interesting half. The useful findings are two empirical results `coder`
already paid for, and one convergence that arrived from both directions at
once.

This reads `~/work/coder` as reference material only. Nothing was copied;
mechanisms are described at the level needed to rebuild them here.

## The convergence worth leading with

`coder`'s plugin gate and our calibration gate were built by different people
for different payloads, and **they are failing in the same shape.**

| | Wins the metric it targets | Loses a different one | Verdict |
| --- | --- | --- | --- |
| `coder`'s `shell_digest` | prompt tokens −35.6% | seconds +10.1% against a 10% guard | reverted |
| our choice adapter | accuracy 0.77 → **0.90** | log loss 1.952 → 2.323, confident errors 1 → 9 | *"ship it for the decision, not yet for the probability"* |

Two independent systems, two payloads, one failure mode: **the candidate wins
the thing it was built to win and pays for it somewhere the author was not
looking, and an honest gate refuses it.** That convergence is worth more than
any amount of machinery reuse, because it says the multi-metric gate is not
bureaucratic caution — it is catching a real and recurring effect.

The diagnosis on `coder`'s side is sharper than ours and we should steal it.
Their plugins that summarized long output genuinely shrank the output, and
*"the model reads a digest and then spends the tokens it saved on more
rounds."* The saving was real and the system absorbed it. We should be asking
the equivalent question about a more accurate adapter: **where does the
accuracy go?** Ours went into confidence, which is the same story told in
calibration terms.

## The result that should change what we build

`coder` ran the experiment we would otherwise have to run, and published the
null.

**Model-called capabilities were never adopted.** `repo_context`: **zero
calls across 18 attempts on six task shapes**, in runs that made 15 to 99
shell calls each. `repo_search_bounded`: **zero calls in six of six**. The
diagnosis is mechanical rather than mysterious — the model kept reaching for
the general tool it already had, and the specialist sat behind a generic
dispatcher with an untyped input until a schema was fetched.

And the offer was not free. Declaring the capability cost **2,307 extra JSON
bytes on every request of every turn**, whether or not it was ever called.

Host-driven capabilities, which fire because the host decided they should,
at least ran and produced measurable effects — the whole table above is made
of them.

So: **put the call site in our code, at points we choose, and never offer a
decision model to an agent as a tool it may elect to use.** If one sentence
survives from this page into the architecture, that is the one. It is also an
argument PAW gets right by construction — `fn = paw.function("triage");
fn(text)` is an ordinary function call in ordinary code, not a tool a model
elects.

The corollary, from `coder`'s own routing analysis: a small local function
earns its place by **removing a model call, not by advising one.** A parser,
an index lookup, or a classifier that *answers* deletes a round. That is the
mechanism behind every improvement that stuck.

## Where the analogy holds

**The artifact/identity split is identical.** PAW ships weights plus a
run-time scaffold; a plugin ships an artifact plus a manifest that is the
whole grant — identity, a pinned digest, typed input and output schemas, and
what the thing may reach. A compiled function's signature would sit in those
schemas unchanged.

**We had already converged on the schema.** `docs/kev/mesh-plan.md` defines a
decision-model artifact manifest with adapter digest, size, format, rank, and
targets, a catalog row, and the rule *"a row without a measured `evalRef`
does not admit."* That is `coder`'s release row rediscovered for weights,
with the measurement reference made mandatory. Two designs reaching the same
schema from opposite payloads is the real evidence that the socket is a
genuine abstraction.

**Listing separate from release.** Display copy mutable, bytes write-once,
one current-release pointer, rollback as a pointer change so a run's evidence
keeps naming a row that exists. We have no version scheme at all today beyond
run-directory names, and `creatorDefined` is empty in all three of our
packages.

**`measurement` as a first-class column.** Their catalog ships an honest
negative beside the artifact — the measured win *and* the measured cost, and
"not in the default suite." We are already writing those sentences by hand in
our calibration records. They belong in the row.

**Revocation with a bounded enforcement delay, which we need now.** A client
whose policy snapshot is older than 24 hours stops running managed artifacts
entirely, so a revocation reaches even a machine you cannot contact. Our
documented failure mode is that an OS update changes the base signature and
invalidates every adapter and every calibration map fitted against it. **That
is a revocation event with no mechanism.** We do the load-time half — the
door refuses to start on a signature mismatch — and have no remote half at
all.

**One owner per decision point.** Their rule table gives each trigger exactly
one owning plugin and refuses a second that claims it. Apple's runtime takes
an adapter *or* a use case, never both, so for us that stops being tidiness
and becomes the only arrangement the runtime permits.

## Where it breaks

**Purity, but not the way it first looks.** The objection is not that an
adapter needs a resident base. It is that **a code plugin's purity is provable
at admission by inspection and an adapter's is not.** Their loader reads the
compiled module's import list before instantiating anything and refuses
anything undeclared — the sandbox is a property of what was loaded. A
`.fmadapter` has no imports. There is nothing to read. Its purity is a
property of the runtime that attaches it.

Our own mesh plan already says this better than I can: there is no artifact
to verify for a hosted base, so *"the verification floor has to move from
digests to behavior."* That is the break, and we found it first.

**Size, and it is not close.**

| | |
| --- | --- |
| Their typical plugin | 117–146 KB |
| Their largest real plugin | 7.15 MB |
| Their artifact ceiling | **8 MiB** |
| A PAW adapter, quantized | ~23 MB |
| **Our `.fmadapter` weights** | **133 MB** |

Sixteen times over the ceiling — and the ceiling is not incidental. Artifact
bytes are a database column, length-checked before decode. Raising it is not
a constant change, it is a different storage design: content-addressed blob,
digest and size in the row.

**Composition, with the objection reversed.** Two code plugins do not compose
trivially either — they compose because a rule table forbids overlap, and
where the table cannot help, their own docs concede defeat and admit one at a
time until the combination is measured. So the mechanism we would want already
exists and is already the answer to a hard problem. What is worse for weights
is only the degree: PAW's hot-swapping is sequential, with no switching cost
published, and our runtime cannot hold two adapters at all.

**Measurement, where the carry runs backwards.** Their conformance is exact
packet equality — the same input returns the same bytes a year from now. A
decision model's conformance is a distribution over a held-out split, and
nothing in that toolchain expresses it.

More importantly, **do not carry their admission test.** It requires a
candidate to be no worse on the mean of any metric across every task —
point estimates, **no noise model at all** — against a bench whose
control-against-control spread runs 63% to 158%. The predictable result: their
default suite has never held a member, and one candidate that swept all three
metrics on all five held-out tasks correctly counted for nothing because the
control's own spread covered it. They diagnose this themselves and prescribe
the interval they have not yet built.

Our gate is further along, not behind: margins rather than point estimates,
a held-out requirement, a minimum sample, per-family granularity, and a typed
refusal on the wire when it has not been passed. The lesson is to **add the
interval, not the sweep** — which is exactly what #9375 and #9376 are doing.

## What to do

Nothing here justifies a dependency, and PAW still does not serve our
contract (see [`2026-09-19-compiled-functions.md`](2026-09-19-compiled-functions.md)).
Three things are worth taking.

1. **Write the manifest for the adapter we already have.** No new training.
   Name, version, artifact digest and size, base signature and minimum OS
   build, the contract's input and output schemas, estimator and sample
   count, and an `evalRef` pointing at the committed calibration records with
   their verdicts. This makes four checks scattered across three binaries into
   one gate keyed off one document.

2. **Replace static admission with a behavioral floor.** Digest match, then
   base-signature match, then the isolation probe passes, then a per-family
   admitted calibration record exists — otherwise serve the typed answer with
   probabilities omitted and refuse `uncalibrated`. Every piece exists; none
   of them is keyed off a single manifest. *Status: built in
   `crates/lev/src/admission.rs` (#9389); see
   [`../../lev/manifest.md`](../../lev/manifest.md#where-the-last-issue-attaches).*

3. **Prove revocation, because it is the one thing we demonstrably lack.**
   Publish the manifest with a freshness window, mark a release revoked, and
   show a running door stop serving that family — including a door that never
   reaches the service again, which must stop within the window regardless.
   That is precisely the mechanism the base-signature treadmill needs.

Everything else — a catalog UI, publishing scopes, a global namespace, upload
idempotency — is premature. There is one adapter, one owner, and one machine.

One thing to avoid: their capability search ranks a query against a name and
description by bag-of-words token overlap. That is the ad-hoc keyword routing
our own contract forbids, and it is worth noting that a system this carefully
built still reached for it at the one point nobody was measuring.

## A correction to the premise

The plugin suite being empty is not evidence of retreat. The default set has
**never** held a member in any commit — it was introduced empty, and
"reverted" in those records describes an A/B verdict rather than a source
revert. The emptiness is the gate's only output so far, produced by a rule
stricter than the instrument could justify. That is a different and more
interesting failure than a design that was tried and abandoned.
