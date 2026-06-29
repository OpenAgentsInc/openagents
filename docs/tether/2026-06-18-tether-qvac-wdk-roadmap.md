# Tether, QVAC, WDK, And OpenAgents

Date: 2026-06-18

Status: analysis and sequencing memo. This is not a product promise, partner
commitment, wallet integration commitment, or settlement-authority change.

## Sources Reviewed

- All current `docs/launch/` launch plans and audits through the June 18
  roadmap.
- All current `docs/tassadar/` research, run-state, training-gap, marketplace,
  executor, payment, and capability-envelope notes.
- Local Tether reference lane at `/Users/christopherdavid/work/projects/tether/`,
  including top-level OpenAgents/Tether strategy memos and the cloned public
  `tetherto` repos for QVAC, QVAC examples, QVAC Fabric, QVAC native extension
  repos, QVAC vcpkg registry, WDK, and Tether MDK.

## Executive Recommendation

OpenAgents should treat Tether/QVAC as validation and reference material, not
as a replacement substrate.

QVAC proves that local-first, P2P, cross-platform AI runtime work is real and
well-funded. WDK proves that Tether is building wallet orchestration with
default-deny policy hooks and protocol modules. Tether MDK proves that Tether
is also building miner telemetry and orchestration infrastructure.

The right OpenAgents response is narrower and more receipt-driven:

1. Keep Spark as the primary agent balance and Lightning payout path while the
   v1 settlement loop hardens.
2. Keep OpenAgents MDK/Money Dev Kit in the checkout and treasury lane already
   described by launch docs.
3. Do not add Tether WDK as live payout authority before the real settlement and
   auto-payout gate is stable.
4. Add a WDK-compatible receipt/export adapter only after OpenAgents receipts,
   recipient confirmation, and real-only settled totals are boring.
5. Use QVAC/Fabric/extension repos as study and Rust-port pressure for
   Psionic, Autopilot Voice, Pylon/Nexus artifact distribution, and Autopilot
   Coder repo-understanding.
6. Use Tether MDK as a later flexible-load/miner telemetry reference, not a
   pre-v1 dependency.

The sequencing rule:

```text
receipt authority first
  -> payout loop stability
    -> non-authoritative wallet adapters
      -> edge compute enrollment pilots
        -> stablecoin/customer-payment experiments
          -> institutional flexible-load integrations
```

## Why Not Add WDK Now

WDK is beta wallet infrastructure. Its root package is a small JavaScript ESM
orchestrator that registers wallet managers, protocols, middleware, and policy
rules. The most interesting design point is not "use WDK for payments." It is
the default-deny policy engine on governed accounts, simulation support, and
the separation between root orchestration and chain/protocol modules.

That pattern is useful, but live settlement is the wrong place to import it
today.

OpenAgents is still closing the v1 settlement discipline:

- Spark is the primary agent balance.
- OpenAgents MDK is scoped to checkouts and treasury support.
- Public projections must stay evidence-only and stale-safe.
- Real sender-side settlement is not the same as recipient-confirmed receipt.
- The real-only settled total must not mix simulation and real payments.
- Gate 2 real settlement and autonomous auto-payout was the largest remaining
  release gate in the launch docs.

Adding WDK as a live money rail before those facts are boring would broaden the
wallet and settlement surface while the product is still hardening the current
one. That creates more audit burden without improving the immediate launch
claim.

## When To Add WDK

Add WDK in three steps, and stop after any step that fails receipts or policy
review.

### Step 1: Now, Study Only

Keep WDK in the Tether reference lane. Extract design lessons:

- small root orchestrator;
- explicit wallet/protocol registration;
- governed accounts;
- default-deny operation policy;
- simulation before execution;
- structured policy violation errors;
- disposal/teardown for sensitive wallet state.

Do not import `@tetherto/wdk` into OpenAgents product code yet.

### Step 2: After Gate 2, Non-Authoritative Receipt Adapter

After real payout and auto-payout are stable, design an
`ExternalWalletReceiptAdapter` that can export or display OpenAgents payment
receipts in an external wallet surface.

Constraints:

- OpenAgents receipt refs remain authority.
- Spark/Lightning remains the supply-side payout authority unless explicitly
  superseded by a later invariant and product-promise change.
- WDK may show a linked receipt, destination address, external account label, or
  user-facing payout summary.
- WDK must not be allowed to mark OpenAgents work as accepted, settled, or
  recipient-confirmed.
- Any wallet policy engine use must be simulated first and covered by a test
  fixture.

This can become a yellow product promise only after a self-test receipt proves
the adapter behavior without raw secrets.

### Step 3: Later, Customer Payment Or Stablecoin App Rail

Stablecoins are useful for customer payment, app payment, and enterprise
billing. They are not the best first denominator for supply-side compute
settlement.

The correct split is:

```text
customer payment rail: flexible, including card, dollar, stablecoin, or Bitcoin
contributor settlement rail: Bitcoin-first for accepted compute work
```

If demand appears, WDK can be evaluated as one customer/app wallet surface. It
should not replace Bitcoin/Spark as the compute-contributor payout path without
a separate legal, product-promise, accounting, and invariant change.

## What To Add First

### 1. Tether/QVAC Study Packet For Autopilot Coder

The safest immediate addition is not a wallet dependency. It is a study packet.

Create a machine-studying corpus around the local Tether lane:

- source manifest of Tether/QVAC/WDK/MDK repos reviewed;
- architecture questions for QVAC plugin boundaries, worker isolation, model
  identity, registry, RAG, and delegated inference;
- hidden repo-edit exam that asks Autopilot Coder to modify an adapter or doc
  without vendoring upstream code;
- answer key focused on OpenAgents authority boundaries.

This matches the launch roadmap: machine studying is internal dogfood and Forge
Autopilot Coder acceleration, not a public training claim.

### 2. Psionic Capability Manifest Pressure

QVAC's strongest runtime lesson is capability declaration:

- model family and version;
- artifact digest and expected size;
- tokenizer/config digest;
- source kind: filesystem, HTTP, registry, P2P, cache, or bundled;
- backend admission;
- quantization tier;
- benchmark refs;
- refusal reasons.

Add this to Psionic-owned manifests before adding foreign runtime code. QVAC
Fabric, MedPsy-style models, Parakeet, Whisper, TTS, Stable Diffusion, and
BitNet are good pressure tests for the manifest.

### 3. Autopilot Voice Provider Contracts

QVAC's speech stack is broad: Whisper, Parakeet ASR, EOU, Sortformer
diarization, Chatterbox/Supertonic TTS, GGUF metadata dispatch, and local
benchmarks.

OpenAgents should port the contract shape, not adopt QVAC as voice authority:

- `SpeechToTextProvider`;
- `TextToSpeechProvider`;
- `VoiceActivityOrEndOfUtteranceProvider`;
- provider metadata with model/artifact/backend facts;
- fixed audio benchmark corpus;
- shadow comparison against current providers;
- no-cloud local dev mode.

Autopilot Voice keeps session, pane, command, approval, and receipt authority.
Psionic owns runtime metadata and benchmarks.

### 4. Pylon/Nexus Artifact Availability Records

QVAC's Hyperdrive/Hyperswarm paths are useful as artifact distribution design
pressure. They are not job authority.

Add OpenAgents-owned records for:

- cached model shard availability;
- artifact digest and source;
- device capability class;
- integrity-check work classes;
- local/offline fetch strategy;
- refusal when artifact, device, or backend facts do not match.

Nexus/Pylon must keep admission, assignment, validation, accepted-work
accounting, and receipts.

### 5. External Wallet Receipt Adapter Design

Write the schema before code:

```text
ExternalWalletReceiptAdapter
  input: OpenAgents accepted-work receipt ref
  output: external wallet display/export payload
  authority: none
  may read: public-safe payout summary, destination label, receipt URL/ref
  may not write: accepted status, settled status, recipient confirmation
```

This is where WDK belongs first.

### 6. Flexible-Load Work Classes

Tether MDK is a Mining Development Kit for miners, sensors, pools, power
meters, containers, and facility telemetry. It is more relevant to the
miner/data-center flexible-load lane than to user wallet flows.

Later, after v1 payout stability, define work classes for:

- eval/replay;
- artifact integrity;
- benchmark probes;
- embedding batches;
- local inference batches;
- adapter micro-windows;
- checkpoint validation;
- interruptible batch jobs.

Then evaluate whether MDK-style telemetry and command contracts can inform
miner or facility adapter design.

## What Not To Add

Do not add WDK as a primary payout wallet before Gate 2 real settlement and
auto-payout are stable.

Do not make WDK, USDt, or any external wallet surface the source of truth for
accepted work, real-only settled totals, or recipient-confirmed settlement.

Do not adopt `@qvac/sdk` as Autopilot's runtime spine. QVAC is an inference and
distribution SDK. OpenAgents is a governed work, receipt, review, and payout
system.

Do not let QVAC P2P discovery become job admission, worker trust, model trust,
or payment authority.

Do not vendor large QVAC/Fabric/extension code into OpenAgents. The Tether lane
is read-only reference material by default.

Do not present mobile phones as frontier-training workers. Start with eval,
replay, artifact, benchmark, and small inference work classes.

Do not present MedPsy-style models as clinical products. Treat them as model
family, artifact, safety, quantization, and benchmark tests.

Do not create a public Tether/QVAC/WDK product promise until the adapter or work
class has self-test receipts and legal/compliance review.

## Recommended Timeline

### Before Stable v1

Do:

- keep this analysis in docs;
- create the Tether/QVAC study packet for Autopilot Coder;
- define Psionic capability manifest fields that QVAC pressures;
- define the external wallet receipt adapter schema;
- keep WDK as reference only;
- keep Tether MDK as reference only;
- continue proving real payouts and public-safe projections.

Do not:

- add WDK to live settlement;
- advertise stablecoin payouts;
- route OpenAgents product authority through QVAC;
- start a broad runtime rewrite.

### After Gate 2 Real Settlement Is Stable

Do:

- build a local-only WDK receipt-export spike;
- run it against redacted accepted-work receipts;
- test default-deny wallet-policy behavior in fixtures;
- publish no product claim unless the receipt packet is public-safe.

Decision gate:

```text
Can an external wallet surface display an OpenAgents receipt without becoming
OpenAgents settlement authority?
```

If no, stop.

### After Autopilot Coder Study Infrastructure Works

Do:

- add Tether/QVAC/WDK/MDK repo corpus manifests;
- add hidden exams around QVAC plugin architecture, WDK policy simulation, and
  MDK telemetry contracts;
- use results to improve Autopilot Coder's codebase understanding path.

Decision gate:

```text
Does studied repo knowledge improve real code-change performance without
creating public training claims?
```

If no, keep it internal.

### After v1 Payout And Receipt Loop Is Boring

Do:

- design an opt-in compute enrollment pilot for local-first or wallet apps;
- start with desktop Pylon/Psionic Level 0 or Level 1 work;
- show accepted work, rejection reasons, earnings, pause controls, and receipts;
- keep contributor payout Bitcoin-first.

Possible partner-facing sentence:

```text
Your users can earn Bitcoin for accepted AI work while your app keeps its own
wallet, privacy, and local-first experience.
```

### After Miner Or Facility Pilot Scope Exists

Do:

- review Tether MDK worker contracts as telemetry/control references;
- define miner/data-center capability manifests;
- model interruptible and checkpointable workloads;
- measure validation cost per accepted result;
- keep grid/flexible-load claims separate from current consumer-compute proof.

## Product-Promise Posture

Current state should stay internal/red:

```text
tether.qvac_reference_lane.v1: internal reference only
wdk.receipt_export_adapter.v1: planned, no production dependency
tether_mdk.flexible_load_reference.v1: planned reference, no production path
```

A future yellow promise is acceptable only after:

- a schema exists;
- a fixture exists;
- no raw secrets are exposed;
- OpenAgents receipt authority remains intact;
- public copy distinguishes real from simulated settlement;
- legal/compliance signs off on stablecoin or wallet claims.

## Final Decision

Add WDK later, not now.

The immediate OpenAgents work is to make the receipt and payout loop stronger,
then let WDK sit outside it as a non-authoritative wallet-display or
customer-payment adapter. QVAC and Tether MDK are more valuable today as study,
benchmark, capability-manifest, and future compute-enrollment references than
as dependencies.

OpenAgents should become the layer that assigns, validates, governs, pays for,
and receipts useful AI work. Tether/QVAC can be a distribution and runtime
reference for that future, but OpenAgents contracts must decide what work
means.
