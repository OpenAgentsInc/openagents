# Codex analysis: what should catch the Coldcard class of failure

Label: **Codex analysis**

Status: independent defensive analysis. This is not a finding, ProductSpec,
implementation plan, authorization to scan a target, disclosure decision, or
public vulnerability claim.

Basis: the Loupe reference study and preliminary Omega scan in
[`README.md`](README.md),
[`2026-07-31-omega-first-scan-preliminary.md`](2026-07-31-omega-first-scan-preliminary.md),
and
[`2026-07-31-omega-first-class-pentester-speculation.md`](2026-07-31-omega-first-class-pentester-speculation.md),
together with the Coldcard account in
[`../coldcard/chatgpt-pro-analysis.md`](../coldcard/chatgpt-pro-analysis.md).
This analysis accepts the question's assumption that Loupe would not have
found the Coldcard vulnerability before exploitation. That is a counterfactual
assumption, not an observed Loupe result.

## Conclusion

The Coldcard vulnerability should have been caught by a build-time assertion
about the final firmware artifact: the wallet-seed path must resolve to the
approved hardware entropy provider, and the software fallback must not supply
the required symbol. A source scanner could have raised several clues, but the
decisive check is not “does secure RNG code exist?” It is “which exact code
does this exact release build call when it creates a secret?”

That distinction should set the direction for Bitcoin security work. Defenders
should scan not only for locally suspicious lines, but for failures that reduce
an attacker's search space while leaving outputs, types, builds, and ordinary
tests looking normal. The most dangerous combination is a small or structured
secret space plus a cheap public validation oracle. AI does not create that
vulnerability, but it can compress the work required to discover the
cross-repository path, reproduce the generator, enumerate candidates, and test
them against public data.

OpenAgents should therefore not build a second general-purpose source scanner.
It should retain Loupe as one candidate generator and build the missing
defensive system around it: compiled-artifact provenance, security-invariant
manifests, cross-repository variant analysis, executable evidence, isolated
adversary simulation, continuous regression watch, and controlled disclosure.
The first deliverable should be a small artifact-provenance witness, evaluated
blindly against historical builds and seeded fixtures before any larger
workbench is justified.

## Why this was outside Loupe's strongest path

Loupe's useful discipline is real. Findings are emitted through typed tools,
the discovery agent must propose a regression-test diff, a separate verifier
commits its verdict before attempting a repair, semantic and deterministic
deduplication coexist, and reporting can stop behind a human approval gate.
Those are sound controls for model-produced findings.

But Loupe's analysis unit, as documented at commit `c94aac5`, is one source
file. Its `validate_poc` check establishes that a diff applies; it does not run
the proposed test. Loupe does not build an exact board configuration, inspect
the link map, trace a security-critical call through several repositories,
exercise the resulting firmware, or prove the provenance of bytes returned at
runtime. The preliminary Omega scan reinforces the evidence boundary: a
plausible, well-written candidate remained only a pending claim until its test
was actually executed.

The Coldcard failure crossed every one of those seams. A migration changed seed
generation from a direct board RNG call to `ngu.random.bytes()`. A zero-valued
configuration macro was interpreted as “defined” by one dependency and
“disabled” by another. The board code did not export the exact global
`rng_get()` symbol expected by libNgU. The linker therefore selected an
ABI-compatible MicroPython fallback. The correct hardware RNG implementation
was still present, the firmware built, the returned bytes varied, and hashing
made them look normal. The defect existed in the relationship among
configuration, dependency semantics, symbol ownership, link selection, and
runtime reachability. No single line had to look catastrophic.

On the assumption in the question, Loupe misses because its evidence boundary
stops before the fact that matters. This does not prove that an agent could
never infer the bug from source. A sufficiently capable session with the right
files and build context might. It means that the documented architecture does
not mechanically require the inference or verify it in the final artifact.

## What would have caught this vulnerability

The most direct control is the one embodied by the eventual fix: inspect the
objects or link map and fail the build unless the board-specific object defines
the global `rng_get()` used by the release, while the upstream fallback object
defines no competing RNG symbols. This assertion is deterministic, cheap, and
attached to the artifact that will ship. It converts an architectural belief
into a release invariant.

A second control is configuration-semantics analysis. Numeric Boolean macros
must be tested by value when value is the contract. A checker should flag
security-sensitive cases in which one component uses `#ifdef` or `#ifndef`
while another uses `#if FLAG`, especially when `FLAG` is defined as zero.
Preprocessing the supported board configurations would have exposed that
libNgU's guard passed while MicroPython compiled its fallback. This is a useful
early warning, but it is weaker than the artifact assertion: a corrected guard
does not prove that the intended symbol ultimately won.

A third control is end-to-end security-path provenance. For each secret-creating
operation, the project should declare the permitted sources, transformations,
and sinks. For Coldcard, a machine-readable invariant would say that wallet
seed bytes must be reached from an approved hardware or secure-element entropy
source, must retain a stated minimum amount of independent input, and must not
pass through a noncryptographic fallback. The build should resolve this claim
over preprocessed source, compiler inputs, objects, symbols, and the linked
image. A source-level call graph alone is insufficient because the linker's
choice was the failure.

A fourth control is a fault-injection build matrix. Remove or disable the
approved entropy provider and require the build to fail closed. Build every
supported board and release configuration. Deliberately set security flags to
undefined, zero, and one where those are supposed to have distinct meanings.
Change link order. Include and exclude fallback objects. A cryptographic device
that still builds and creates a seed after its approved entropy provider is
removed has demonstrated a serious invariant failure even before anyone knows
the exact exploit.

A fifth control is runtime provenance testing on the real target or a faithful
instrumented build. The test should record that a seed-generation request
reaches the approved device source and that source failures abort secret
creation. It should not try to certify security by looking at output
statistics. A deterministic PRNG and a hash can produce bytes that pass
distinctness, repetition, and statistical-randomness checks. Those tests can
detect a stuck source; they cannot prove entropy origin or quantify independent
secret input.

Finally, security review should treat a migration across a cryptographic API as
a change to the entire security path, even when the new dependency has a strong
reputation. A before-and-after witness should compare the source of key, seed,
nonce, and blinding randomness in the compiled products. The relevant question
is not whether the new library is reputable, but whether the integration
preserves the old security property.

No one of the heuristic checks is enough. The reliable chain is: declare the
invariant, evaluate it in the exact configuration, prove it in the final
artifact, make the negative case fail closed, and preserve the check in every
release.

## What catches related vulnerabilities

The Coldcard mechanism defines a broader family of dependency-integration and
artifact-composition bugs. Defenders should look for same-name, same-signature
symbols supplied by multiple objects; weak symbols and interposition; link-order
dependent behavior; default implementations that silently satisfy a missing
platform hook; dead or unreachable security code whose mere presence can fool
review; and source-level calls whose final targets change by board, feature,
toolchain, or release mode.

Configuration deserves its own variant family. Search for defined-versus-true
confusion, zero-valued feature flags, double negatives, defaults that differ
between dependencies, generated headers that shadow board settings, and
security checks that disappear when a value is absent. Compile the real
configuration matrix instead of reasoning only about the default developer
build. Compare the preprocessed output and artifact graph across configurations
so a secure debug build cannot conceal an insecure release build.

Entropy and secret-generation analysis must follow information, not names.
Identify values derived only from identifiers, clocks, counters, process state,
public constants, or correlated sensors. Flag deterministic generators used as
cryptographic sources, repeated state without fresh entropy, partial-state
reseed operations, narrow integer conversion, slicing or truncation after a
wide hash, and mixers that combine two predictable or correlated values. Track
the effective width retained at every boundary. Hashing, XOR, checksums, and
random-looking encodings must not be credited with creating entropy.

The same analysis extends beyond master seeds. It should enumerate every
consumer of the affected source: signing nonces, ephemeral keys, channel keys,
authentication challenges, encryption keys, masks, backups, passwords,
session identifiers, and device-cloning material. One weak primitive often has
a larger blast radius than the first user-visible feature that exposed it.

Projects also need oracle analysis. A reduced candidate set becomes much more
dangerous when each guess can be checked cheaply and without rate limits. In
Bitcoin, addresses, xpub-derived addresses, signatures, and the public chain can
serve as exact or near-exact validation oracles. Other open-source systems have
analogues: public keys, password verifiers, ciphertext structure, signed
artifacts, predictable identifiers, public API responses, and reproducible
protocol transcripts. A scanner should connect “secret has at most N plausible
states” to “public evidence distinguishes the right state,” rather than rating
either fact in isolation.

Dynamic tools catch other adjacent classes. Coverage-guided fuzzing is well
suited to parsers, serializers, state machines, and unsafe memory boundaries.
Property and differential tests are better for cryptographic and consensus
semantics. Sanitizers catch memory and undefined behavior. Reproducible builds,
binary diffing, symbol allowlists, and software-composition records catch
unexpected artifact changes. Fault injection catches unsafe fallbacks. None of
these replaces whole-path provenance; together they prevent a local scanner
from becoming the sole safety argument.

## What open-source projects should scan for in an AI-assisted attack era

Projects should assume that an attacker can cheaply ask agents to traverse
several languages and repositories, explain unfamiliar build systems, generate
emulators, translate old PRNGs, write GPU kernels, enumerate common wallet or
protocol paths, and correlate candidates with public data. There is no evidence
in the cited Coldcard analysis that AI was used in the actual theft. The point
is defensive: AI reduces the integration labor that once protected obscure,
cross-layer defects through inconvenience.

The highest priority is therefore not “bugs an AI might invent.” It is
**machine-amplifiable leverage already present in public code**. Projects should
scan for these combinations:

- a security-critical value with a smaller, more correlated, or more
  enumerable state space than its interface suggests;
- a public, offline, or weakly rate-limited oracle that confirms guesses;
- an ambiguous configuration, ABI, FFI, serialization, or dependency boundary
  where both sides type-check but assign different meaning;
- a fallback that keeps the system available by silently weakening security;
- a source-to-sink path that crosses repository, generated-code, build-script,
  linker, firmware, or hardware boundaries;
- attacker-controlled data reaching interpreters, shells, subprocess options,
  file or URL handlers, parsers, authorization decisions, signatures, or key
  material;
- security assumptions that are documented or tested only by code presence,
  successful compilation, output appearance, or happy-path behavior;
- a historical fix, fork, vendored copy, or reimplementation whose variants
  were not searched across the ecosystem.

Scan prioritization should follow consequences and attacker economics. Rank
secret creation, signature and verification, firmware update trust, consensus,
transaction construction, wallet import/export, PSBT handling, network parsers,
authorization gates, subprocess execution, and remotely reachable resource
allocation ahead of uniform alphabetical file coverage. For each high-value
path, ask four questions: what property is promised, what exact compiled code
supplies it, how can the negative case be made to fail, and what oracle would
let an attacker validate a candidate cheaply?

Continuous scanning matters because the dangerous moment is often a benign
refactor or dependency update. Delta analysis should expand beyond changed
lines to callers, callees, generated configuration, build inputs, resolved
symbols, and downstream consumers. Once a vulnerability is confirmed, its
structural pattern should become a private variant query over maintained
branches, forks, vendored copies, and sibling projects. The query should be
shared carefully enough to help defenders without publishing an exploitation
recipe before affected maintainers can act.

## What OpenAgents should build

OpenAgents should build a **security-invariant and evidence workbench**, with
Loupe as one input rather than the product boundary. Loupe already provides a
good candidate lifecycle. The missing product is the machinery that binds a
candidate to an exact build, proves or falsifies it, searches related projects,
coordinates remediation, and keeps the fix alive.

The core object should be a versioned security-invariant manifest. It binds a
target repository and commit, dependency commits, toolchain, build
configuration, expected security source and sink, allowed symbol providers,
forbidden fallbacks, minimum retained widths, negative-test behavior, and the
commands that produce evidence. Its result must be `satisfied`, `violated`, or
`not proven`; absence of evidence must never collapse to “safe.”

An artifact witness should consume preprocessed sources, compiler commands,
object files, archives, link maps, firmware images, and debug metadata where
available. It should answer narrow questions such as: which object supplied
this symbol; which implementation is reachable from this secret-creation
entry point; which configured fallback is present; where was a value truncated;
and did a dependency or toolchain change alter the answer? The witness should
emit a compact, reproducible receipt, not a model narrative.

Above that witness, OpenAgents should add an attack-surface and variant engine.
It should rank sensitive paths, let Loupe and other analyzers propose
candidates against coherent cross-file slices, and translate a confirmed root
cause into searches across revisions and repositories. Model diversity can be
measured, but it must not be mistaken for independence: two model families
reading the same incomplete evidence can agree on the same error. Independent
verification means a separate execution identity and a mechanically distinct
evidence step.

The workbench should execute regression tests, property tests, fuzz harnesses,
and permitted impact demonstrations in disposable Linux VMs. A candidate moves
from prose to evidence only when the system observes the required failure on
the vulnerable artifact and the corresponding success after repair. For an
entropy weakness, the strongest permitted demonstration is reconstruction of a
synthetic or owner-controlled test secret in a local Bitcoin regtest fixture.
It must never search for, derive, or spend a live user's key.

Finally, OpenAgents should build the operation around the evidence: private
triage, encrypted maintainer contact, embargo state, cross-operator dedup by
nonrevealing commitment, regression-pack delivery, release watch, budget
accounting, and signed receipts. This is where the existing Loupe analysis is
most persuasive: coordination, verification, remediation, and persistence are
the differentiators, not another prompt that asks a model to find bugs.

## Phased build plan

### Phase 0: authority, threat model, and benchmark

Define the allowed target classes, rules of engagement, data retention,
disclosure policy, budget caps, and evidence vocabulary before adding scanning
capacity. Build a benchmark from historical vulnerable and fixed commits,
synthetic symbol-collision/configuration fixtures, and clean controls. Keep the
Coldcard-shaped fixture separate from the detector authors for a blinded replay
where practical. The gate to continue is evidence that the proposed checks
find historical failures without an unusable false-positive rate. A
hindsight-written rule that recognizes only this one macro and symbol is not a
product.

### Phase 1: artifact witness MVP

Support one C/C++ embedded build pipeline and a small invariant schema. Ingest
the exact build configuration, preprocessed source, dependency revisions,
objects, and link map. Implement symbol-provider assertions, forbidden-fallback
assertions, secret-source-to-sink reachability, and width/truncation tracking.
Add fault builds that remove the approved provider and must fail. Produce a
reproducible receipt tied to the artifact digest. Dogfood only on OpenAgents-
owned fixtures or explicitly authorized projects.

### Phase 2: Loupe adapter and executable evidence

Let Loupe findings and independent analyzers attach invariant proposals and
runnable regression packs to the same typed record. Execute them in disposable,
network-restricted Linux VMs with pinned toolchains. Require observed failure on
the vulnerable state and success on the repaired state. Preserve Loupe's
verdict-before-patch ordering, conservative severity, typed submission, and
human approval. Add a real `not proven` state for missing hardware, incomplete
build context, or unresolved provenance.

### Phase 3: entropy, oracle, and variant analysis

Add domain analyzers for entropy sources, mixers, reseeds, retained bit width,
secret consumers, and validation oracles. Search supported histories, forks,
vendored dependencies, and reimplementations for a confirmed pattern. Add
commit-delta watch over security paths and final artifact graphs. Evaluate
precision on other seeded classes so the system does not overfit Coldcard.

### Phase 4: controlled dynamic defense

Add agent-generated fuzz and property-test harnesses for prioritized boundaries.
Permit end-to-end demonstrations only in synthetic, regtest, signet, emulator,
or owner-controlled hardware labs. Make network access, target credentials,
exploit-artifact export, and compute spend separate typed capabilities. A
dynamic result advances evidence; it never grants disclosure or broader test
authority.

### Phase 5: coordinated service and persistence

Add private campaign rooms, verified maintainer contacts, encrypted disclosure,
embargo tracking, nonrevealing cross-operator dedup, repair review, and
regression-pack watch across releases. Only after the internal workflow has
survived several authorized disclosures should OpenAgents consider a broader
ecosystem service. Success should be measured by confirmed findings per unit of
cost, time to maintainer acknowledgement and repair, false-positive burden,
variant coverage, and persistence of fixes—not by the number or severity of
model-written findings.

Each phase must stand on its own and may falsify the next. In particular, if
artifact witnesses cannot be made reproducible across real embedded builds, or
if attack-surface prioritization does not outperform simpler baselines, the
architecture should change before fleet scale is added.

## Defensive and authorization boundaries

Repository contents are hostile input to an AI scanner. Source comments,
documentation, tests, and generated files can contain prompt injection or tool
instructions. Workers must treat repository text only as data. System policy
and rules of engagement must be immutable from the target; tools must be
allowlisted; source should be mounted read-only; writes should go to disposable
storage; credentials should be absent; network access should default off; and
all finding, verdict, patch, and evidence transitions should use typed,
server-validated calls. Loupe's bubblewrap posture is the floor, not the whole
boundary; higher-risk execution warrants a disposable VM per target.

Static inspection of public code is not equivalent to authorization for every
action that analysis makes possible. OpenAgents should scan code it owns, code
whose maintainers opted in, or public code only within an admitted responsible-
disclosure program and applicable legal and platform constraints. Accessing a
private target, probing a deployed service, testing third-party infrastructure,
correlating candidate private keys with real owners, or using nonpublic data
requires separate explicit authority. A license to read source is not a license
to exploit a system. This analysis is not legal advice.

Impact validation must remain in a lab: never mainnet, never live funds, never
third-party nodes or devices, and never real user keys. Use known synthetic
seeds, owned fixtures, regtest or signet networks, bounded candidate spaces,
and nonvaluable test outputs. Keep exploit artifacts and detailed reproduction
data encrypted and access-controlled. Retain the minimum data required to
reproduce a finding, and exclude secrets, device identifiers, user data, and
provider credentials from model prompts and public receipts.

Discovery, verification, repair, maintainer notification, publication, and
release are distinct authorities. A model finding authorizes none of the later
steps. A verifier should be independent of the producer, but verifier status
does not authorize disclosure. Public claims, including the statement that a
named project may be vulnerable, require sufficient executed evidence, human
review, maintainer handling under the disclosure policy, and the authority
required by `AUTHORITY.md`. Settlement, bounty claims, fund movement, and any
attempt to use a recovered secret remain outside this analysis.

Budgets and stop conditions are security controls too. Every campaign needs a
pinned target, bounded scope, immutable rules of engagement, compute and token
caps, retention deadline, and a kill switch. An inconclusive result should stay
inconclusive or expire visibly; it must not silently become either a clean bill
of health or a public accusation.

## Uncertainty and falsifiers

This analysis is written after the vulnerability was known and is therefore
exposed to hindsight bias. The Coldcard-specific artifact assertion would have
caught the documented mechanism, but that does not establish that a general
system would have proposed the right invariant before the incident. The proper
test is a blinded historical replay using only information available at the
time, plus unrelated vulnerable and clean projects.

The cited Coldcard document does not establish the attacker's exact program,
the precise method used to constrain every device's state, or that every swept
address came from Coldcard. Practical enumeration cost depends on device,
firmware, UID knowledge, timing, and RNG-call history. Claims about AI reducing
attacker labor are reasoned threat-model assumptions, not attribution for this
incident.

Static provenance can prove which software implementation a build selects; it
cannot by itself prove that physical entropy is healthy, independent, or
unobservable. Hardware measurements, circuit review, fault testing, and supply-
chain assurance remain separate disciplines. Conversely, statistical output
testing cannot prove software provenance. A defensible release needs both where
the threat model requires them.

The existing Loupe documents do not prove that Loupe would miss this defect,
and their preliminary Omega results do not establish a verified detection rate.
They do prove narrower architectural facts: per-file fan-out, apply-only PoC
validation, and the absence of whole-program artifact, dynamic, continuous,
and ecosystem-variant evidence. The recommendations above are intended to fill
those exact gaps. They should be rejected or revised if measured benchmarks do
not show better detection, evidence quality, and cost than the simpler Loupe
baseline.
