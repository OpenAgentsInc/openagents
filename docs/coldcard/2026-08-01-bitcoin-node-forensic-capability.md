# Our Bitcoin node as forensic capability

Status: **capability analysis, implementation proposal, and first measured
results.** It authorizes no scan of a third-party target, no public claim, no
attribution, and no change to the managed-sandbox credential boundary. It
records read-only reproduction and a 1,701-block scan against our own node, and
proposes how that node should — and should not — be wired into the forensic
workbench.

Date: 2026-08-01

**Headline results (§9).** All eight published known-positive vectors reproduce
exactly. The fingerprint's false-positive rate — never previously published — is
2,820–6,154 per million at 2 sat/vB and 24–701 per million at 30 sat/vB, which
is high enough to change how much the fingerprint can carry. The published
"estimate always overshoots the real size" property does not reproduce. No
undocumented sweep wave was found in 1,701 blocks, including the previously
unscanned pre-wave rehearsal window.

Reading order:

- [`chatgpt-pro-analysis.md`](chatgpt-pro-analysis.md) — anatomy of the defect.
- [`2026-08-01-kelbie-independent-postmortem-analysis.md`](2026-08-01-kelbie-independent-postmortem-analysis.md)
  — the postmortem we are reproducing, and the sentence this document answers.
- [`../loupe/2026-08-01-omega-forensic-analysis-roadmap.md`](../loupe/2026-08-01-omega-forensic-analysis-roadmap.md)
  — §6.2 reproduction pack, Phase 6, and the placement rules that bound
  everything below.

Subject repository: `Kelbie/coldcard-rng-postmortem`, local read-only clone at
`~/work/projects/repos/coldcard-rng-postmortem`, synced 2026-08-01. No LICENSE
file: quotable, not vendorable.

---

## 1. The sentence this document answers

Our own analysis of the postmortem, written before this check, said of its
detector `tools/coldcard-node-scan.py`:

> I read this tool; I did not run it. It requires a full unpruned archival node
> we do not have, and running third-party code was out of scope.

The first half of that is no longer true. We have the node. It has been running
since 2026-02-12, and until this week it was carrying $634/month as backing for
a Lightning stack that has since been swept and decommissioned. It is now an
idle archival full node with no RPC clients.

That makes it the cheapest unlock in the forensic program: the one piece of
Phase 6 infrastructure that normally costs a month of sync and a terabyte of
disk is already built, already validated, and already paid for.

The rest of this document is about what it can actually prove, what it cannot,
and how to wire it in without breaking the sandbox credential boundary.

---

## 2. What the node actually is

Observed live, 2026-08-01, read-only over IAP:

| Property | Value |
| --- | --- |
| Instance | `oa-bitcoind`, `n2-standard-8`, `us-central1-a` |
| Core version | `/Satoshi:30.2.0/` |
| Chain | mainnet |
| Blocks / headers | 960,596 / 960,596 — zero behind |
| Verification progress | 100%, `initialblockdownload: false` |
| Pruned | **no** — full archival |
| `txindex` | **enabled** |
| Size on disk | 864 GB of a 2 TB SSD (48% used, ~983 GB free) |
| Peers | 10 |
| RPC posture | `rpcbind` on loopback and `10.42.0.2`, `rpcallowip=10.42.0.0/24`, `disablewallet=1` |

Two properties matter more than the rest.

**Unpruned.** The postmortem's scanner reads whole blocks across ranges. A
pruned node cannot answer for historical ranges at all, which is why the tool
states the requirement in its header.

**`txindex=1`.** The scanner's header says a transaction index is *optional*
because block scanning carries what the fingerprint needs, and that a
half-built index "costs a column rather than the run." Having it complete is
strictly better: it resolves arbitrary prevouts without walking blocks, which
is what the evidence-graph suite (§6.2 suite 4) needs for payer discovery, and
what makes late prevout resolution cheap in the OFR-016 candidate funnel.

`disablewallet=1` is also worth stating plainly: this node has never held a
wallet, has no keys, and cannot spend. It is a read-only chain oracle. That is
the correct posture for forensic use and should not change.

---

## 3. Live reproduction: the fingerprint holds on our node

I ran the postmortem's fingerprint arithmetic against two of its eight self-test
vectors, using only `getrawtransaction` on our node. I did not execute the
third-party script; I reimplemented the arithmetic it documents, which is the
posture §6.2 requires anyway ("recompute them from immutable raw evidence").

The rule under test: fee divided by an *estimated* vsize of
`42 + (68 | 91 | 148) per input by script type` must come out at an exact whole
number of satoshis per vbyte.

| Transaction | Block | Inputs | Fee (sat) | Est. vsize | Rate | Expected | Result |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `2fe075cf0ec799f3…` | 960,189 | 5 × P2WPKH | 11,460 | 382 | **30.000000** | 30 | PASS |
| `44e622cb32d9e65a…` | 960,367 | 1 × P2WPKH | 220 | 110 | **2.000000** | 2 | PASS |

Both also satisfy the two supporting marks:

- **`nLockTime = 0`** on both, where Core and Electrum write the current height.
- **No low-R grinding.** Of the six signatures across the two transactions, five
  are 72-byte DER (33-byte R) and one is 71-byte (32-byte R). Core, Electrum and
  Sparrow all pay a retry to avoid the extra byte.

This is a real result: an independent party, on independent infrastructure,
recomputing the postmortem's discriminating fingerprint from raw chain data and
landing on the same exact integers. It is the first rung of the historical
chain-fingerprint suite, cleared at a cost of about two minutes.

### 3.1 One discrepancy, recorded not smoothed

The postmortem's central claim about the estimator is that it **overshoots the
real signed size every single time** — that is the part which proves the fee was
priced from a table before the transaction existed. On these two transactions it
did not overshoot. The estimate exactly equalled the real vsize (382 = 382,
110 = 110).

The likely explanation is benign: both are all-P2WPKH, and 68 vbytes is the
per-input cost of a P2WPKH spend with a maximum-length 72-byte signature, so the
table value and reality coincide when the signature is full-length, with integer
vsize rounding absorbing the 71-byte case. The overshoot would then show up on
mixed or legacy script types, and on inputs whose signatures grind shorter.

I am not asserting that explanation — I am recording that our two-sample
reproduction confirmed the *exact-integer rate* property but did **not**
independently confirm the *strict overshoot* property, and that the difference
is exactly the kind of thing OFR-016's mutation controls and script-type
stratification exist to settle. A reproduction that quietly reported "PASS" on
all three marks here would have been laundering.

§9 settles this at scale: on all eight published known-positive vectors, and on
893 of 905 fingerprint matches inside the documented wave blocks, the estimate
**equals** the real vsize rather than exceeding it.

---

## 9. Wide scan: 1,701 blocks, 7.12M transactions

Executed 2026-08-01 on `oa-bitcoind` at chain tip 960,601. Three ranges, all
read-only, exact integer satoshi arithmetic, zero prevout errors:

| Scan | Blocks | Range | Eligible tx | Purpose |
| --- | --- | --- | --- | --- |
| `incident` | 420 | 960,180–960,599 | 1,782,741 | all documented waves plus everything after |
| `prewave` | 980 | 959,200–960,179 | 4,736,538 | the unscanned pre-wave rehearsal window |
| `control2025` | 301 | 900,000–900,300 | 603,465 | negative control, unrelated era |

Self-test first, per OFR-016: **all eight published known-positive vectors
reproduce** — seven at exactly 2 sat/vB, one at exactly 30 sat/vB. No wide scan
was allowed to run until that passed.

Raw artifacts. These were node-local when this section was written. They are
now frozen content-addressed in Cloud Storage under
`gs://openagentsgemini-oa-artifacts/forensics/coldcard/ofr-016/wide-scan-2026-08-01/sha256-<digest>/`,
and the per-block eligibility and match counts they fold to are checked into
the repository as `fixtures/forensics/coldcard/historical-wide-scan-ledger.v1.json`,
where the §9.1 table below is recomputed from the retained counts rather than
asserted:

| File | SHA-256 |
| --- | --- |
| `cc-hits-incident.jsonl` | `a5fec35b3da3da8ce01ac1325fdfbdddd51bff5e72859a0c9875e7ed6c59676e` |
| `cc-hits-prewave.jsonl` | `f081763cbe904954d231ee7683d772934010a750e857f8a21cf56aef7daaa813` |
| `cc-hits-control2025.jsonl` | `d7d15ee6ad356b2d7a8a5547164ab455ad2e291358ca0f3bf331c08452626e78` |
| `cc-stats-incident.jsonl` | `b2f3c2e55ac0491825739dafcc332a161a50e9e0309c054b2e1c188a724f9da9` |
| `cc-stats-prewave.jsonl` | `a3ee450edbe653d9059afaa6e466882a05efd13d341cc41913f3feef37849b0e` |
| `cc-stats-control2025.jsonl` | `ea4ffbea2de9b6f0e8e04e62e2d712d1a5f437327d29478cd8d1e70fde349fcb` |

Range endpoints are checkpointed by block hash — e.g. `incident` spans
`00000000000000000001bb5454b2ff5a4c954adad88e748cef281da1af9d482e` to
`000000000000000000005e8a53cdc76c07f460649b19ac4c9782fba2e4030484`.

### 9.1 The unpublished denominator

This is the number that did not exist before, and it is the main result.

False-positive rate per million eligible non-coinbase transactions, by era:

| Era | Eligible tx | rate = 2 | rate = 30 | any integer rate |
| --- | --- | --- | --- | --- |
| `control2025` | 603,465 | **2,820** | **537** | 37,043 |
| `incident` | 1,782,741 | **3,707** | **701** | 25,876 |
| `prewave` | 4,736,538 | **6,154** | **24** | 18,922 |

Across all 7.12M transactions, **2.22%** satisfy the exact-integer fee rule at
some whole-number rate. The rule alone carries almost no information.

Two consequences follow, and both correct the published account.

**At 2 sat/vB the fingerprint is unusable.** Between 2,820 and 6,154 ordinary
transactions per million match. In the 980-block pre-wave window alone that is
roughly 29,000 chance matches. The postmortem is candid that 2 sat/vB collides
with ordinary traffic; this measures the collision at three to six per thousand.

**At 30 sat/vB the fingerprint is far weaker than reported.** The postmortem
reports "zero false positives across 7,553 transactions at 30 sat/vB." Our
control era yields 537 per million. At that rate, 7,553 transactions have an
expected false-positive count of about **4**, and observing zero is an ordinary
outcome of a small sample — not evidence of a clean discriminator. The reported
zero is consistent with our measurement; the inference usually drawn from it is
not. Scanning 1.78M transactions in the incident window at 701 per million
implies on the order of 1,200 chance matches at rate 30 before any real sweep is
counted.

### 9.2 The overshoot claim does not survive

Stratified over every match, not just the self-test set:

| Population | n | estimate = real | estimate > real | estimate < real |
| --- | --- | --- | --- | --- |
| Published known positives | 8 | **8** | 0 | 0 |
| Matches in documented wave blocks | 905 | **893** | 8 | 4 |
| Rate-30 consolidation candidates | 25 | 23 | 1 | 1 |
| 2025 control era | 8,048 | 7,647 | 77 | 324 |

The published claim is that the estimate exceeds the real signed size **every
single time**, and that this is what proves the fee was priced before signing.
On our independent implementation it essentially never does: the estimate lands
exactly on the real vsize in 98.7% of documented-wave matches and in all eight
published vectors.

This does not refute the fingerprint — the exact-integer property reproduces
perfectly, and that is the part doing the work. It refutes the stated
*mechanism* for why the property holds, or indicates the reference
implementation's size table differs from the documented `42 + 68|91|148`.
Either way, the overshoot property should not be cited as evidence until it is
reconciled.

### 9.3 Candidate sweeps outside the documented waves

Applying the full mark set — exact integer rate at an operator constant,
`nLockTime = 0`, single output, all-P2WPKH inputs, no low-R grinding, at least
two inputs, at least 0.01 BTC — reduces 158,111 raw matches to **25
transactions in 15 blocks** at rate 30.

The documented waves light up exactly as they should: blocks 960,183 / 960,185 /
960,188 / 960,190 / 960,191 carry 15 of those 25, including the 31.39 BTC and
17.45 BTC clusters. The filter finds what it is supposed to find.

The remaining candidates do **not** look like victim sweeps. The largest all
share a shape that is wrong for the theory:

| Block | Rate | Inputs | Value | Fee |
| --- | --- | --- | --- | --- |
| 960,080 | 2 | 2 | 99.56000000 BTC | 356 sat |
| 960,103 | 2 | 2 | 98.95000000 BTC | 356 sat |
| 960,577 | 2 | 2 | 84.21092752 BTC | 356 sat |
| 960,405 | 30 | 2 | 30.15862326 BTC | 5,340 sat |
| 960,240 | 30 | 2 | 5.04994000 BTC | 5,340 sat |
| 960,253 | 30 | 2 | 2.79994000 BTC | 5,340 sat |

Every one is exactly two inputs to one output, with an identical fee inside each
rate class, and several carry round-number values minus a fee
(`5.04994`, `2.79994`). That is the signature of **exchange or custodial
withdrawal automation** — a batcher paying from a fixed size table — not of a
sweep consolidating heterogeneous victim UTXOs. A real sweep looks like the
documented ones: varied amounts, varied input counts, arriving in bursts.

**No credible undocumented wave was found in 1,701 blocks.** That includes the
pre-wave rehearsal window, which nobody had scanned and which was the highest
expected-value region in the plan. It is a negative result, and it is worth
exactly what it is: evidence that this fingerprint, at this width, does not
surface additional operator activity in these ranges — not evidence that none
exists.

### 9.4 What this run establishes and does not

Establishes:

- our node independently reproduces the published fingerprint on all eight
  known positives, from raw chain data, with exact integer arithmetic;
- the fingerprint's false-positive rate, measured for the first time, is high
  enough that at 2 sat/vB it cannot support any claim, and at 30 sat/vB it
  supports a claim only in combination with shape, clustering, and value;
- the published overshoot property does not reproduce;
- 1,701 blocks contain no additional sweep-shaped operator-rate cluster that
  resembles the documented waves.

Does not establish:

- that any listed candidate is a theft, an attacker transaction, or connected to
  Coldcard at all. Every one is a **program-similarity candidate** and nothing
  more. Section 7's limits apply in full: the shape of a transaction is a claim
  about software, never about a person, an intent, or an authorization.
- that the documented waves are complete. Our scan covers 959,200–960,599 and a
  2025 control; earlier rehearsal, later resumption, and other fee regimes remain
  unscanned.
- any figure fit for publication. This is a candidate dataset pending
  deterministic replay and admission, per §7.

---

## 4. What a node buys that a public API cannot

The postmortem is explicit that its own reach was limited by the public
explorer: the API "could only ever hand over a couple of blocks at a time."
That limit is not cosmetic. It is what let wave 4 go unnoticed — 45.9 BTC across
1,126 victim addresses on 31 July, *after* the advisories, missed because
Block's scan stopped at block 960,230 and priced the fingerprint at the wrong
constant. The repository's own FAQ says the second wave "already came, and it
was missed by looking in the wrong blocks for the wrong number."

Five capabilities follow from owning the node, ordered by forensic value:

1. **Whole-range scanning at arbitrary fee rates.** The discriminating test is
   cheap per transaction and embarrassingly parallel. Scanning a 10,000-block
   range locally is bounded by disk, not by someone else's rate limiter. This is
   the capability that finds later waves and the one that measures base rates.
2. **Pre-wave rehearsal detection.** The scanner header calls this out as "the
   interesting one": test transactions *before* the first wave, where an
   operator rehearses. Nobody has scanned that range with this fingerprint,
   because doing so requires exactly what we now have.
3. **Base-rate measurement — the honest denominator.** OFR-016 requires false
   matches reported "per million eligible transactions by fee rate, era, script
   type, and fingerprint revision." That number cannot be obtained from a
   couple of blocks at a time. It requires sweeping large negative control
   ranges, and it is the single most important input to whether the fingerprint
   means anything at all. The postmortem is candid that at 2 sat/vB the
   fingerprint collides with ordinary traffic; only a wide local scan can say
   *how much*.
4. **Prevout resolution at graph scale.** Evidence-graph traversal
   (`victim → transaction → destination → payers → onward spend`) needs
   arbitrary historical lookups. With `txindex` complete, these are index hits.
5. **Mempool visibility.** `--mempool` in the reference tool checks what is
   unconfirmed right now. This matters only while the situation is live — and
   as of 31 July it was still live.

---

## 5. How we should use it — the boundary problem

Here is the tension that governs the design, and it is not optional.

OFR-016's acceptance criteria state:

> The guest receives no external IP, node cookie, RPC credential, wallet RPC,
> provider credential, or arbitrary endpoint.

The roadmap repeats it in §6.2. The forensic worker is a disposable GCE VM with
broker-only networking, and it must stay that way. Meanwhile our node lives at
`10.42.0.2` and accepts RPC from `10.42.0.0/24`.

**The wrong design — and it is the obvious one — is to let the forensic sandbox
reach the node.** Opening a path from a guest that runs model-directed code to
an archival node's RPC surface would hand that guest an ambient, uncontrolled,
un-receipted network capability. It would violate the stated criterion directly,
and it would do so in the specific way that is hardest to notice later, because
everything would appear to work.

The right shape inverts the direction of trust: **the node is an evidence
source, not a service the sandbox calls.**

```
oa-bitcoind (archival, no wallet, no keys, no guest access)
      │  read-only RPC, loopback only
      ▼
Extractor (OpenAgents-owned, outside the sandbox)
  • typed scan profile: network, genesis, required block hashes,
    node version/posture, ranges, fingerprint revision, thresholds
  • pins block hashes + capture time + response digests
      │  emits
      ▼
Content-addressed block-range bundle  (canonical, SHA-256 digest)
      │  delivered as immutable source-bundle material,
      │  same private-artifact path as OFR-003, zero network allowance
      ▼
Forensic worker (GCE, broker-only, no credentials)
  • runs fingerprint evaluation over frozen input
  • append-only raw hits + block-hash checkpoints
  • deterministic, replayable, resumable
```

This satisfies both halves of the roadmap. §6.2 says to "start with a
content-addressed block-range bundle for deterministic evaluation, then add a
private, brokered Bitcoin Core data capability for wider scans." Our node is
what makes the *first* half possible immediately, without waiting for the
brokered capability to be designed and admitted — and when that broker is built,
the node is the thing it brokers, with the extractor becoming the broker's
implementation rather than a throwaway.

It also delivers a property the public-API path can never have: because the
bundle pins block hashes and response digests, an old result stays meaningful.
A mutable public endpoint can silently redefine what a past scan saw. A frozen
bundle from our own node cannot.

### 5.1 Concrete first increment

Small, and it does not touch production flags:

1. **Freeze the self-test bundle.** Blocks 960,183–960,192 plus 960,359 and
   960,367 (the tool's `KNOWN_WAVE_BLOCKS`), with prevouts for every input in
   the eight known-positive transactions. A few MB. This is the known-answer
   dataset that OFR-016 requires before any wide scan is credible.
2. **Freeze negative-control bundles.** Several block ranges from eras well
   before the defect window, chosen to span fee regimes and script-type mixes.
   These are the denominator for base-rate claims.
3. **Reimplement the fingerprint with exact integer satoshi arithmetic** in the
   adapter, self-testing against (1) before it is allowed to report anything
   from (2). Never `float`; the reproduction above used integer satoshis
   throughout and should be treated as the reference for that.
4. **Then, and only then**, consider wider ranges — the pre-wave rehearsal
   window first, since it is the highest-value unscanned region and it is
   bounded.

Step 3 is where the overshoot discrepancy in §3.1 gets settled, because a
stratified self-test over mixed script types will either reproduce the strict
overshoot or show that the property as published is narrower than stated.

---

## 6. Economics

The node costs about **$634/month** — $284 for the `n2-standard-8`, $340 for
the 2 TB SSD, $10 boot. In the cleanup that preceded this document, roughly
$23,700/month of infrastructure was removed, and the owner explicitly held this
node back from that teardown.

Two observations, neither of which is a recommendation to act now:

- **The machine is oversized for a node with no clients.** An `n2-standard-4`
  would serve fine and save ~$140/month. Scanning is disk- and CPU-bound in
  bursts, so this trades against scan throughput; worth revisiting after the
  first wide scan gives a real profile.
- **The disk is not oversized, and should not be cut.** 864 GB used of 2 TB. A
  1 TB disk would fit today and be uncomfortable within a year. Pruning would
  destroy the exact property that makes the node forensically useful. If cost
  pressure returns, cut the machine type, never the archive.

Against the alternative — provisioning and syncing a fresh archival node inside
the admitted cloud path — this node is roughly a month of wall-clock and a
terabyte of egress already spent. That is the real argument for keeping it: not
that $634 is cheap, but that the sunk sync is worth more than the monthly.

---

## 7. What this cannot establish

These are the limits that already bind the program, restated because a node
makes it easy to forget them.

- **A fingerprint establishes program similarity only.** OFR-016 says it "cannot
  establish person, intent, theft, or live-wallet scope." A transaction shape is
  a claim about the software that built a transaction. It is not a person, not a
  motive, and not proof that a movement was unauthorized.
- **Chain movement is not theft without victim evidence.** The roadmap's Phase 6
  gate requires that "program fingerprint, entity grouping, unauthorized
  movement, and identity attribution cannot satisfy each other's gates." Four
  separate claims, four separate evidentiary bars.
- **A scan result is not a source-defect finding.** Everything in this document
  concerns the *event*, not the *bug*. The code-to-artifact suite is a different
  lane with a different verifier, and the postmortem itself never builds the
  firmware.
- **Zero hits is not evidence of absence** unless the run proves it had the data.
  The worker must fail loudly on missing fee or prevout data. A clean-looking
  empty result from an incomplete scan is the exact failure mode Episode 264
  taught us to fail closed on.
- **Nothing here authorizes publication.** Derived data stays a candidate until
  deterministic replay and admission pass, and public claims remain owner-gated.

---

## 8. Recommendation

Keep the node. Treat it as forensic infrastructure rather than residual
Lightning backing, and record it as the evidence source behind OFR-016's
"content-addressed historical block bundles" so it stops looking like an
unattributed $634/month line item.

The first work is bundle extraction and an exact-arithmetic fingerprint
reimplementation with a known-answer self-test — both outside the sandbox, both
cheap, and both directly on the OFR-016 path. The node never becomes reachable
from a forensic guest; it feeds frozen, digest-pinned bundles into one.

The open question this document was written to pose — the false-match rate per
million eligible transactions at 2 and 30 sat/vB — is now measured in §9.1, and
the answer materially weakens what the fingerprint alone can support. That is
the most useful thing produced here, and it came from one afternoon on a node we
already owned.

Three follow-ups, in priority order:

1. ~~**Freeze the artifacts.**~~ Done 2026-08-03 under OFR-016. The raw hit and
   per-block stat files are content-addressed in Cloud Storage, the 1,701-block
   per-block ledger with its block hashes is checked into the repository, and
   blocks 960,189 / 960,359 / 960,365-960,367 are frozen as content-addressed
   bundles carrying all eight published known positives. The measurement no
   longer depends on the machine.
2. **Reconcile the overshoot discrepancy.** Either the published mechanism is
   overstated or the reference size table differs from the documented one. It is
   cheap to settle and it affects how the fingerprint is described.
3. **Widen the base-rate measurement across fee regimes and eras**, since a
   single control era is thin support for a denominator this important.
   Extending it is now purely a matter of scan hours.

None of this requires the sandbox to reach the node, and none of it should
change that.
