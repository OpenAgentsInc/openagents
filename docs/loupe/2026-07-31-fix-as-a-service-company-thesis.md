# Fix-as-a-Service: agentic security remediation at ecosystem scale

Status: **speculation and strategy.** Not a plan, not a ProductSpec, not
authority for any surface. Nothing here authorizes work, spend, third-party
scanning, PR submission, or any public claim. `AUTHORITY.md`, `INVARIANTS.md`,
and the disclosure norms in [`README.md`](README.md) §7 keep full precedence.

Market figures below are order-of-magnitude reasoning marked as such. Where a
number is load-bearing it is flagged **[verify]**. Do not quote any of them
externally without checking.

Companion documents:

- [`README.md`](README.md) — what Loupe is, read at `c94aac5`.
- [`2026-07-31-omega-first-class-pentester-speculation.md`](2026-07-31-omega-first-class-pentester-speculation.md)
  — the technical architecture: eight layers, the evidence ladder, the campaign
  shape. **Read that first.** This document assumes it and does not repeat it.
- [`2026-07-31-omega-first-scan-preliminary.md`](2026-07-31-omega-first-scan-preliminary.md)
  — the live scan of our own code that grounds every claim here.

The architecture document asks *"what would a first-class pentester workbench
look like?"* This one asks a different question: **what is the company, what
does it sell, why does it win, and what kills it?**

---

## 1. The thesis in one paragraph

The security industry sells **findings**. Findings are a cost centre: every one
must be triaged, reproduced, prioritized, and eventually fixed by an engineer
who did not write the bug and does not want the interruption. The scarce
resource was never detection — it is **remediation capacity**. Agents change
that constraint for the first time, but only if their output can be trusted
without human re-derivation, and today it cannot. So: build the thing that
delivers **merged fixes with executed proof**, continuously, across thousands
of projects at once. Sell the outcome, not the report. The unit of value is a
pull request that lands, carrying a regression test observed failing before the
change and passing after.

**The one-line version: the industry sells findings; we sell fixes that are
proven and that stay fixed.**

---

## 2. Why remediation and not detection

Detection is close to solved and getting cheaper every quarter. Our own scan
demonstrates it: an off-the-shelf harness produced 80+ candidate findings
against 70 files of our code in about an hour, unattended, at commodity cost.
Detection is becoming a feature of the model, not a product.

What has not moved is the other side of the ledger. A finding's journey to
"fixed" runs through: triage, reproduction, prioritization, assignment,
context-rebuilding by a stranger to the code, a fix, review, regression
coverage, merge, backport, release. **Every step is human, and the first three
are pure loss** — they produce no code, they only decide whether the finding was
real.

That asymmetry is the whole business. More findings *worsen* the problem: a
scanner that doubles its recall doubles the triage bill. Security teams have
rationally responded by tuning scanners down, ignoring whole severity bands, and
treating backlogs as permanent. Ask any AppSec lead what fraction of their
open findings will ever be fixed; the honest answer is a small one. **[verify]**

Fix-as-a-Service attacks the loss steps directly:

| Step | Today | With executed proof + candidate fix |
| --- | --- | --- |
| Triage | Human decides if real | **Eliminated** — PoC ran and failed on HEAD |
| Reproduction | Human rebuilds context | **Eliminated** — reproduction *is* the artifact |
| Prioritization | Guess from severity label | Evidence tier, not a label |
| Fix authoring | Human, cold context | Candidate PR attached |
| Review | Human | Human — **and this stays human, permanently** |
| Regression coverage | Usually skipped | Ships with the fix |
| Persistence | Nobody watches | Continuous re-verification |

Review stays human deliberately. A service that removes the human merge
decision is not a security product, it is a supply-chain attack with a
subscription.

---

## 3. The false-positive tax, and why it is the wedge

The industry's dominant tax is **false positives**, and the AI wave has made it
worse, not better. Open-source maintainers have been publicly vocal about a
flood of confident, plausible, wrong AI-generated vulnerability reports —
`curl`'s maintainers have been among the loudest, describing the review burden
as a denial-of-service against the project. **[verify: specific quotes/dates]**
Some projects now discard AI-attributed reports on sight.

That reaction is rational and it is a moat-shaped opportunity. The bar to
distinguish yourself is no longer "find more"; it is **"never waste a
maintainer's time,"** and that is a mechanically enforceable property rather
than a cultural aspiration.

This is exactly what the evidence ladder is for, and it is why the ladder is
the commercial primitive rather than an engineering nicety:

- **T0** — an agent's claim. Free to produce, worth nothing, and currently what
  the market is drowning in.
- **T1** — a PoC diff that applies. What Loupe delivers today.
- **T2** — the PoC **executes and is observed failing on HEAD**, passing with
  the fix. Machine-checkable, third-party re-runnable.
- **T3–T4** — dynamic reproduction; end-to-end impact in a sandboxed lab.
- **T5** — fix merged upstream *and* the regression pack still passing N
  releases later.

**The commercial rule: nothing below T2 is ever sent to a human outside the
company.** Internal candidates below T2 are inventory, not product. That single
policy is the difference between being the company maintainers trust and being
another source of slop.

It also inverts the industry's sales motion. Legacy vendors sell finding
*volume* — dashboards full of red. We would sell **fewer items with more
evidence**, and the pitch is "every item we send you is real, and here is the
command you can run yourself to confirm it before you read another word."

---

## 4. The product primitive: a proof-carrying fix

The atomic artifact is not a report. It is a signed bundle:

```
FixBundle {
  target        { repo, commit, subsystem }
  finding       { class, CWE, location, dataflow }
  reproduction  { test diff, harness, container digest, exact command }
  observation   { failed on HEAD @sha, passed with patch, timestamps, runner id }
  patch         { minimal diff, rationale, blast-radius note }
  regression    { permanent test, donated under the project's license }
  provenance    { signature, model matrix, effort, cost, verifier verdicts }
  disposition   { embargo state, disclosure timeline, credit }
}
```

Three properties make it commercially interesting, and each is a deliberate
design choice rather than a side effect:

**It is independently re-executable.** Anyone can take the bundle, run it in a
clean environment, and watch the test fail on HEAD. Trust is not required. That
is what lets it cross an organizational boundary — to a maintainer, an auditor,
an insurer, a regulator — without a relationship existing first.

**It is portable and content-addressed.** A bundle is a bearer artifact. It can
be handed to a customer, escrowed during an embargo, published after
disclosure, or used as evidence in a compliance filing. Signed provenance —
which model families, at what effort, at what cost, with which verdicts — turns
"an AI found this" into an auditable chain.

**It carries its own future.** The regression test is the part that keeps
paying. It is donated to the project, and it is also retained as a permanent
detector: run against every future release of that project, and against every
*other* project with the same shape.

---

## 5. Scale: the multiplication that makes it a company

One project is a consulting engagement. The thesis only works at ecosystem
scale, and scale changes the economics in three compounding ways.

**Variant multiplication.** A confirmed bug shape becomes a hunt pattern. The
Coldcard entropy-truncation class — "full entropy chopped before expansion" —
either exists or does not exist in every wallet that forked or reimplemented
that code. One confirmed root cause, N projects checked, one disclosure wave.
The marginal cost of checking project N+1 for a *known* shape is a rounding
error against the cost of discovering it the first time. **This is the single
highest-leverage operation in the entire design.**

**Corpus compounding.** Every T2+ bundle yields four durable assets: a
detector, a fuzz seed, a benchmark item, and a labelled training example of a
*real, executed* vulnerability with its fix. That corpus cannot be scraped,
because it is defined by execution rather than by text. Its value grows
superlinearly with size — variant analysis is combinatorial in the number of
known shapes times the number of watched targets.

**Fleet amortization.** Attack-surface mapping (L0), harness construction, and
lab topologies are per-*ecosystem* costs, not per-project. Build the Rust
crypto-wallet attack-surface template once; apply it to every wallet. The
second project in an ecosystem is dramatically cheaper than the first, and the
fiftieth is nearly free.

At scale this stops being a scanner and becomes something closer to an
**immune system for a software ecosystem**: shapes learned in one organism
confer resistance across the population.

---

## 6. Unit economics

Illustrative, and every number here is a hypothesis to be measured, not a
claim. **[verify all]**

**Observed cost basis.** Our own run: ~99 files, one agent session per file at
frontier-model xhigh effort, ~80 candidate findings in ~70 files, on a single
n2-standard-8. Compute cost is trivial next to inference cost; inference is the
entire bill.

**Where cost goes down:**
- L0 attack-surface ranking replaces alphabetical file-walking. The
  architecture doc hypothesizes 10–100x; even 5x transforms the model. **This
  is the load-bearing unvalidated assumption of the whole business** and should
  be the first experiment run.
- Model-tier routing: cheap models for triage and dedup, frontier models only
  for the slices that rank highest.
- Corpus dedup: as the shape library grows, an increasing share of findings are
  cheap pattern matches rather than expensive discoveries.

**Where cost goes up (honestly):**
- L2 execution requires building and running the target — for some projects
  that is heavier than the analysis.
- L4 lab environments are real infrastructure per topology.
- Verification is one full agent session *per candidate*, and candidate volume
  is high. Verification may cost more than discovery.

**The comparison that matters.** A human pentest engagement is priced in the
tens of thousands and delivers a point-in-time report with no fixes and no
persistence. **[verify]** A senior engineer's time to triage-and-fix a single
real vulnerability, fully loaded, is plausibly in the hundreds to low thousands
of dollars. If a proof-carrying fix can be produced for materially less than
that, the value capture is obvious and outcome pricing becomes possible.

**Outcome pricing is the unlock.** Nobody can sell "per fix merged" today
because nobody can prove a fix works. T2 evidence makes it sellable:
- per merged fix, tiered by severity and evidence tier
- per watched target per month for continuous analysis and persistence
- per ecosystem campaign for variant waves
- capacity subscription for large orgs running private targets

Outcome pricing also aligns incentives against slop: revenue arrives on merge,
so sending a maintainer a bad PR costs *us*, not them.

---

## 7. Product lines

Six, in rough order of how early they are sellable.

**7.1 Continuous Fix Service (the core).** Point it at your repositories. It
maps attack surface, scans continuously on every commit, produces T2+ bundles,
opens PRs against your branch protection, and keeps the regression packs
running forever. Priced per target per month plus per merged fix. Land with one
subsystem; expand across the org.

**7.2 Dependency and Supply-Chain Remediation.** Most organizations' real
exposure is code they did not write. The differentiator against existing SCA
tooling — which reports "you depend on a vulnerable version" and stops — is
that this produces **fixes for the dependency itself**, upstreamed, plus a
verified backport or a temporary in-tree patch. This is where the ecosystem
work and the enterprise work become the same work: fixing the shared dependency
is simultaneously a public good and the thing the customer is paying for.

**7.3 Attested Coverage (the original one — see §8).** Signed evidence of where
we looked, how hard, with what, and what we did *not* find.

**7.4 Ecosystem Defense Campaigns.** Foundation-, consortium-, or
insurer-funded waves across a whole class of software — every Lightning
implementation, every hardware wallet, every popular parser. Priced as a
campaign with a budget cap and public receipts. This is the Episode 263
commitment in commercial form, and the natural first proving ground because it
is the domain we actually know.

**7.5 The Workbench (seat product).** Omega as the console for security
researchers: findings that navigate via LSP, PoCs that run in a microVM while
you watch, the campaign room in the sidebar, swarms as threads. The persona is
a pentester supervising fifty agents instead of grepping. Think terminal-for-
offensive-security. This is also the recruiting and credibility surface — the
tool the best researchers want to use is the tool whose company they join.

**7.6 The Benchmark.** A public, executed benchmark built from the T2+ corpus:
real bugs, real repos, real fix-verified outcomes. Whoever owns the standard
for measuring security-agent capability shapes the category and gets enormous
distribution for free. It must be run honestly, including against ourselves,
including when we lose.

---

## 8. Attested absence: the product nobody sells

The most original idea here, and the one most likely to be underrated.

Every security vendor sells **presence**: here is what we found. Nobody sells
**attested absence**: a signed, specific, reproducible record of where we
looked, how hard, with which model families, against which attack classes, and
what we did not find.

That is what assurance actually needs. An insurer underwriting cyber risk, an
auditor signing off, a regulator under the EU Cyber Resilience Act, an acquirer
in diligence, a board asking "are we exposed to this class" — none of them want
a findings list. They want *bounded, evidenced negative results*.

It is sellable now because it is finally producible. Loupe already records
dismissals and inconclusive verdicts as first-class outcomes; the architecture
already emits receipts per stage; the corpus defines what "attack class" means
concretely. The artifact:

```
CoverageAttestation {
  target        { repo, commit }
  scope         { slices examined, ranked by attack-surface reach }
  effort        { model matrix, effort tier, agent-hours, spend }
  classes       { attack classes hunted, with the detectors used }
  results       { confirmed, dismissed-with-reason, inconclusive-with-reason }
  negative      { "no finding of class X in subsystem Y at effort Z" }
  provenance    { signatures, timestamps, re-execution instructions }
}
```

Three reasons this is strategically important beyond its own revenue:

**It monetizes the 90% of work that produces no finding.** Today a scan that
finds nothing is a cost with no artifact. Attested absence turns the null
result into the deliverable — which is exactly the economics that makes
continuous scanning sustainable rather than a loss leader.

**It is honest in the way the domain requires.** "We examined this at this
effort and found nothing" is a true and useful statement. "This code is secure"
is neither. The attestation format makes the honest claim the natural one to
sell, and it inherits our own discipline directly: an honest blocked row is
worth more than a fabricated pass.

**It is defensible.** Reproducing a competitor's finding is easy. Reproducing a
competitor's *coverage claim* requires their corpus, their attack-surface
templates, and their fleet.

---

## 9. Adjacent markets, same machinery

The core loop — map surface, hunt, execute proof, propose fix, prove
regression, persist — is domain-agnostic. Four expansions in descending order
of fit:

**Smart contracts.** The strongest adjacent fit by a wide margin. Audits are
priced in the tens to hundreds of thousands and gate real capital; the tooling
culture is already fork-test-native (Foundry/forge tests against a forked
chain), so **L2 and even L4 are nearly free** — an exploit PoC that drains a
fork is a T4 artifact produced by the same harness that runs a unit test. Value
per finding is bounded by TVL rather than by remediation cost, which makes it
the one domain where the evidence ladder maps directly onto dollars.

**Firmware and hardware wallets.** Precisely the Coldcard class. Harder — emulation, JTAG, side channels — but the highest-stakes and most underserved, and the credibility of doing it well is enormous.

**Infrastructure-as-code and cloud posture.** Enormous market, but crowded and
mostly about misconfiguration rather than novel bugs. The differentiator would
be the same one: proven exploitability in a sandboxed replica of the account,
not a policy-lint warning.

**AI-generated code specifically.** The volume of machine-written code is
rising fast, and it carries characteristic defect shapes. A corpus specialized
in *how agents introduce bugs* — the shapes that survive human review because
they look idiomatic — is a natural specialization, and one we are unusually
positioned to build because we operate agent fleets that write code all day.
Meta-note worth stating plainly: our own session produced exactly such a
defect class — code that shipped, passed tests, and was never called.

---

## 10. The moat

Asked adversarially: *why can a frontier lab, or Semgrep, or a well-funded
startup not do this next quarter?* The honest answers, weakest first:

**Not the model.** Anyone can call the same APIs. Model access is not a moat
and betting on it is a mistake.

**Not the scanner.** Loupe is MIT/Apache and excellent. Assume competitors
adopt its best ideas — locked verdicts, typed emission, fail-on-HEAD PoCs —
because we intend to.

**The execution fleet is a real but shallow moat.** VM-isolated, per-target,
disposable build-and-run infrastructure across many languages and ecosystems is
genuine engineering with a long tail of build-system misery. It buys quarters,
not years.

**The corpus is the deep moat.** A library of executed, fix-verified bug shapes
with working detectors and fuzz seeds cannot be scraped, cannot be synthesized,
and compounds with every campaign. Its value is combinatorial: shapes ×
targets. Ten thousand shapes across a hundred thousand watched repositories is
an asset with no substitute, and it can only be accumulated by *doing the
work over time*.

**Maintainer trust is the deepest moat and the slowest.** In a market being
flooded with AI slop, the right to have your PR read is scarce and earned. It
cannot be bought, it compounds, and it is destroyed by a single bad quarter of
volume-chasing. The company that becomes "the one whose PRs maintainers merge
on sight" holds something no amount of capital replicates. **Every incentive
inside the company must be aligned to protect this, because the natural
pressure of growth targets is to violate it.**

**Coordination position, if L5 happens.** If cross-operator commitment and
dedup become how the ecosystem coordinates disclosure, the operator of that
fabric sits at the centre of the market. Standards positions are winner-take-
most. This is the highest-variance element of the thesis and should not be
counted on.

---

## 11. Tailwinds

- **Regulation.** The EU Cyber Resilience Act imposes vulnerability-handling
  and reporting duties on products with digital elements, phasing in through
  the second half of this decade **[verify exact dates and scope]**. It creates
  a legal requirement for exactly the artifacts in §4 and §8. Similar pressure
  exists in US disclosure rules and federal procurement. Compliance is the
  budget line that converts a nice-to-have into a purchase order.
- **SBOM → VEX.** SBOM adoption produced a downstream problem: knowing you ship
  a vulnerable component says nothing about exploitability, and VEX statements
  are currently vendor assertions. A VEX backed by an **executed** proof — or
  by an attested absence — is a materially better artifact than the assertion
  it replaces.
- **Code volume.** Agent-written code is growing faster than review capacity.
  The gap between code produced and code reviewed is the market.
- **The attacker side already moved.** This is Episode 263's premise, and it is
  the uncomfortable one: defenders who only test against frontier closed models
  are calibrating against the wrong adversary. Attacker-parity — including
  open-weight models — is a product requirement, not a research curiosity.

---

## 12. What kills this

Ranked by probability × severity. A thesis without this section is marketing.

**1. Slop reputation.** One quarter of volume-chasing, one wave of confident
wrong PRs, and the maintainer trust that is the deepest moat is gone
permanently. The T2 floor must be enforced structurally — in the pipeline, in
pricing, in comp — not culturally. This is the primary risk and it is
self-inflicted by construction.

**2. Provider policy.** Model providers restrict offensive-security use, and
terms shift. Mitigations: multi-provider from day one, open-weight capability
in-house (which attacker-parity requires anyway), and a posture that is
demonstrably defensive with authorization and disclosure receipts. A single-
provider dependency here is an existential single point of failure.

**3. Legal and jurisdictional.** Unsolicited scanning of third-party code sits
in genuinely unsettled territory that varies by jurisdiction and by license and
terms of service. Public-code-plus-responsible-disclosure is the conservative
reading but it is a reading. **Get real counsel before the first unsolicited
scan of code we do not own**, and prefer opt-in and foundation-sponsored
campaigns until that is settled.

**4. Liability for fixes.** Shipping a patch that introduces a vulnerability, or
that breaks production, is a different risk class from shipping a report. Human
merge review is the primary control and must never be sold away, however
attractive "autonomous remediation" sounds in a pitch.

**5. Commoditization from above.** A frontier lab ships "find and fix bugs in
your repo" as a platform feature. Genuinely plausible. The defenses are the
corpus, the execution fleet, ecosystem-scale variant analysis, and trust — none
of which a platform feature has. The wrong response is to compete on model
quality.

**6. The economics simply do not close.** If L0 ranking does not beat
alphabetical, and verification costs more than the fix is worth, this is a
services business rather than a product business. **This is knowable cheaply
and should be knowable before serious capital is committed** — see §14 step 2.

**7. Maintainer relations at scale.** Even excellent unsolicited work is
experienced as pressure by volunteer maintainers. The operation's reputation is
set by its first several disclosures, and the temptation to scale contact
volume before trust exists is the trap.

---

## 13. Organizational shape

Three observations that follow from the above rather than from convention:

**The evidence bar is an engineering invariant, not a value statement.** "No
bundle below T2 leaves the building" belongs in the pipeline as a gate that
fails closed. Sales compensation must not be able to route around it. Every
company that has destroyed a trust-based moat did so by making an exception for
a quarter.

**Researchers become operators.** The hire is not "someone who finds bugs" but
"someone who directs fifty agents that find bugs and can judge their output."
The workbench (§7.5) is the tool that makes that person 100x, and building it
well is a recruiting strategy as much as a product one.

**Publish the losses.** The benchmark (§7.6), the dismissal rates, the
false-positive rates, the campaigns that found nothing. In a market defined by
unverifiable claims, being the party that publishes its own negative results is
both differentiating and the only honest posture for a security company. It is
also the same discipline that governs our release gate: an honest blocked row
beats a fabricated pass.

---

## 14. Sequenced path

Each step is independently useful and independently falsifiable. Nothing below
is authorized by this document.

**Step 0 — finish what is running.** Complete the current Omega scan, verify
its findings, and execute the surviving PoCs to T2 by hand on the existing GCE
host. Apply the evidence ladder to ourselves before anyone else. Then stop
`oa-loupe-scanner-1`; it is billable.

**Step 1 — one disclosure, done by hand.** Settle the finding-12 upstream
question and run the full play manually: verify, execute, contact, embargo,
patch, regression pack, credit. One end-to-end disclosure teaches more about
what to automate than any amount of design.

**Step 2 — measure the load-bearing assumption.** Build L0 attack-surface
mapping for one target and measure cost-per-confirmed-finding against the
per-file baseline we now have from the Omega run. **If ranking does not beat
alphabetical, the economics of the whole thesis change and it is better to know
in a week than after a build-out.**

**Step 3 — dogfood at small scale.** Ten public projects we already depend on,
opt-in or foundation-blessed where possible. Produce T2 bundles and real PRs.
Measure the only two metrics that matter at this stage: **merge rate** and
**maintainer sentiment**.

**Step 4 — the persistence loop.** Regression packs running against every
release of those ten. This is the recurring-revenue mechanic and the thing no
competitor is doing; prove it works before selling it.

**Step 5 — first paid campaign.** A bounded ecosystem wave with a budget cap
and public receipts, in the domain we know best. Sell the campaign, not the
platform.

**Step 6 — attested coverage as a product.** By now the corpus and receipts
exist. Package §8 for the first auditor, insurer, or compliance buyer, because
that is the artifact that turns continuous scanning from a cost into a
deliverable.

Only after those does anything resembling a platform, a marketplace, or a
standards position make sense.

---

## 15. Open questions

- **What is the actual merge rate?** Everything hinges on it and we have no
  data. Step 3 exists to produce that number.
- **Does verification cost more than discovery?** If so, the verification
  architecture — not the scanner — is the core engineering problem.
- **How much of the corpus is transferable across ecosystems?** Variant
  analysis within Rust wallets is plausible; Rust → C firmware is an open
  empirical question and the answer bounds §5's compounding claim.
- **Where is the boundary between the ecosystem good and the paid product?**
  Fixing a shared dependency helps a paying customer and everyone else equally.
  That is a strength for trust and a problem for capture, and open-core is the
  obvious answer without being an automatic one.
- **Which entity owns this?** The IDE, the fleet, the Nostr fabric, and the
  cloud sandboxes live in different repos under different authority profiles.
  A security business has liability, insurance, and disclosure obligations that
  may argue for separation. Deliberately unanswered.
- **Do we have standing to make security claims at all?** We are one session
  removed from shipping a spend ceiling that read zero for weeks. That is not
  disqualifying — it is the argument for building this, and for pointing it at
  ourselves first — but it means the first credibility we earn must be earned
  on our own code, in public, including the parts that embarrass us.

---

## 16. The compressed version

Detection is becoming free. Remediation is not. The bottleneck was never
finding bugs, it was trusting a fix enough to merge it — and trust is
mechanizable if you insist on executed proof rather than model confidence.
Build the pipeline that produces proof-carrying fixes, refuse to ship anything
below that bar, point it at ecosystems rather than single repos so that one
confirmed shape multiplies across thousands of targets, keep the regression
packs running forever so the fixes stay fixed, and sell the negative results
too, because attested absence is what assurance actually needs and nobody sells
it. The moat is not the model — it is the corpus of executed proofs and the
right to have your pull request read.
