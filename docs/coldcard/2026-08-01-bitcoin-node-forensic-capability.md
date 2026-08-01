# Our Bitcoin node as forensic capability

Status: **capability analysis and implementation proposal.** It authorizes no
scan of a third-party target, no public claim, no attribution, and no change to
the managed-sandbox credential boundary. It records one live read-only
reproduction against our own node and proposes how that node should — and
should not — be wired into the forensic workbench.

Date: 2026-08-01

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

The immediate open question, worth settling early because it changes how much
the fingerprint can carry: what is the false-match rate per million eligible
transactions at 2 and 30 sat/vB across eras? Nobody has published that number.
We are now, as far as this analysis can tell, unusually well positioned to
measure it.
