# The missing layer is coordination, not scanners

Status: **assessment and speculation.** Authorizes nothing. No public claims
about third parties beyond what cited sources already state publicly.

Written after opening [project-loupe/loupe#37](https://github.com/project-loupe/loupe/pull/37),
which fixes a real defect in a good tool and does not address the thing that
actually failed.

Companion reading: [`2026-08-01-coldcard-prefix-experiment-results.md`](2026-08-01-coldcard-prefix-experiment-results.md)
(the measurement), [`2026-08-01-hardening-against-ai-assisted-attacks.md`](2026-08-01-hardening-against-ai-assisted-attacks.md)
(what to hunt), [`../transcripts/263.md`](../transcripts/263.md) (the
commitment).

---

## 1. Our own experiment is the argument

The Coldcard defect was not found by better analysis. It was found by **running
two configurations and noticing they disagreed.**

Arm A — the default — produced twelve plausible findings and missed it. Arm B
produced twenty-two and found it three times. Same tool, same model, same
commit. **The finding was the divergence.**

A single operator running a single configuration gets arm A and goes home
satisfied. That is not a hypothetical: it is what any Loupe user would have
gotten scanning Coldcard the week before the theft, because the default clone
is bare and a bare clone cannot fetch submodules.

**Nobody captures divergence today**, because scan results are never published,
so no two runs are ever compared. Free signal, on the floor, uncollected.

---

## 2. Open source solved permission and nothing else

Two projects, opposite licences, same outcome: almost nobody read the code
closely enough.

**Coldcard is source-available under a restrictive licence.** A licence that
forbids building on the code destroys the *builder* base, and the builder base
*is* the reader base. Nobody reads code they cannot use. Five years of public
availability produced approximately zero eyes on the linker question.

**Loupe is MIT/Apache and, as far as we can tell, has approximately one user
running it against third-party code: us.** Permissive licensing removed every
legal obstacle and produced no scanning ecosystem at all.

So permission was never the binding constraint. Look at what running a scan
actually costs an individual:

| | |
| --- | --- |
| Compute | Real money. Our Omega run: 100M+ tokens for 99 files. |
| Obligation | A finding creates unpaid disclosure work. |
| Standing | Usually a project you neither own nor depend on. |
| Legal | Unsettled ground for scanning third-party code. |
| Reward | None. No credit, no record, no standing. |

**A rational individual does not run this scan.** Open source removed the
prohibition and left the economics untouched. That is the gap, and no scanner
improvement closes it.

---

## 3. Linus's law was always half-stated

*"Given enough eyeballs, all bugs are shallow"* omits the allocation problem.
Eyeballs do not distribute themselves by risk; they pool where attention
already is — popular projects, recent changes, interesting subsystems. The
Coldcard entropy path was none of those. It was boring, old, and load-bearing.

Agents change one variable: **eyeballs are now cheap and, for the first time,
allocatable.** You can decide where to look and then actually look there, at
scale, on purpose.

That moves the bottleneck from *"can we look?"* to **"who decides where to
look?"** — which is a coordination problem. Episode 263 named it precisely:

> it kind of seems like we have a coordination problem in the Bitcoin
> ecosystem, because there's no coordinated effort for systemic agentic AI
> defense

The scanner was never the missing piece. Loupe already existed, already worked,
and already had the best evidence discipline of anything in the category.

---

## 4. The default-configuration trap is a social failure

The submodule defect is worth dwelling on because of what kind of failure it
is.

It is not subtle. It is one `git clone --bare` and a missing `--recurse-submodules`.
Any operator who had thought about it for ten minutes would have caught it. The
reason nobody did is that **there was no second operator to compare against.**

With ten independent teams scanning Bitcoin projects and publishing what they
ran, somebody runs it with submodules, gets different results, and asks why.
The divergence surfaces the trap within days. With one team, the trap persists
indefinitely and every scan silently covers a third of the program.

This generalizes. **Every tool has a default, every default embeds assumptions,
and a monoculture of one configuration is blind in exactly the shape of that
default.** Diversity of configuration is a security property, and it only
exists if configurations are shared and results are comparable.

---

## 5. What to build

Ordered by how much each unlocks the others.

### 5.1 A public scan ledger

*"This repository, at this commit, was scanned with this configuration, at this
depth, and here is what was covered."*

Signed, content-addressed, append-only. What becomes visible immediately:

- **Which projects have never been looked at**, which is most of them.
- **Which configurations were used** — and that everyone used the default.
- **Where coverage is stale** — scanned once, at a commit from two years ago.

The submodule gap would have shown up here as a social fact ("every scan of
this repo covered 12 files of a 458-file program") long before anyone found it
technically.

This is [attested absence](2026-07-31-fix-as-a-service-company-thesis.md) as
public infrastructure rather than a product feature. It also monetizes nothing
and should probably not try to.

### 5.2 Community scan profiles

The real defect was never "submodules missing." It was **"the default is wrong
and nobody knew."**

A reviewed, versioned profile per ecosystem — *Bitcoin firmware: materialize
submodules, rank entropy and key-derivation paths first, hunt these classes,
here are the known shapes* — is a config file that encodes what took us a week
and a live theft to learn. It is the cheapest possible transfer of hard-won
knowledge, and it turns a technical trap into a reviewable artifact.

### 5.3 Divergence as a signal

When two independent runs over the same target disagree, **the diff is a
lead.** Different model families, different configurations, different scoping —
each blind in a different shape.

Nobody does this because nobody shares results. Once 5.1 exists, divergence
detection is nearly free and is the highest-signal-per-dollar analysis
available.

### 5.4 Campaign coordination on NIP-29

Per Episode 263, and the piece Omega is actually positioned to build:

- **A campaign is a room.** Scanners, verifiers and humans are participants with
  Nostr identities. Findings and verdicts are signed events.
- **Hash-committed findings.** Publish a commitment at discovery; reveal after
  disclosure. Two teams learn they are on the same bug **without either
  revealing it** — solving credit and duplicated spend with no trusted
  intermediary. One event.
- **Embargo state machine** with room-visible timers, and encrypted maintainer
  contact.

The commitment scheme is the load-bearing primitive. It makes cooperation
cheaper than racing, which is the only way a multi-party effort survives its
first contested finding.

### 5.5 Cost pooling

The unglamorous item that decides whether any of the above happens.

Someone funds the compute; runners get credit and standing in the ledger.
Foundation-, consortium-, or insurer-funded campaigns are the obvious shape, and
Episode 263 already asked the right question out loud — *"where's all that grant
money going?"* There is real money in Bitcoin ecosystem security and
approximately none of it is buying systematic agentic review.

Note what this does **not** require: no bounty market, no token, no settlement
rail. Credit in a public ledger is sufficient incentive for a large class of
contributors, and mixing money into disclosure changes the legal and social
shape entirely. Keep it out until someone can articulate why it must be in.

---

## 6. The tension to design for now

**A public coverage map tells attackers exactly where nobody is looking.**

Attested absence is valuable to defenders and is simultaneously a target list.
"No one has ever scanned this wallet's entropy path" is an actionable sentence
for both sides.

Our working answer, offered as a starting point rather than a solution:

- **Aggregate coverage public** — "N wallets scanned for entropy provenance this
  quarter" — which is the number that makes the case for funding.
- **Specific gaps shared inside the coordination group**, where membership is
  cheap to obtain but not anonymous.
- **Confirmed findings under embargo** until disclosure completes, with
  commitments published at discovery so credit does not depend on speed.

This is not obviously right. It is the first hard design question of the whole
programme, and it should be settled before the ledger exists rather than after.

---

## 7. What this means for us specifically

We should be honest that we are one data point pretending to be a trend. Some
discipline on our own claims:

- **We have found no confirmed vulnerability in anyone's code.** Our Omega scan
  produced 132 unverified findings and zero confirmed ones, because Loupe's
  verify stage is broken. The Coldcard hit is a rediscovery of a known,
  published bug on a commit chosen because we knew the answer.
- **The most useful thing we have produced so far is a PR to somebody else's
  tool**, and that is the correct ratio for a group with no track record.
- **We should build 5.1 and 5.2 before anything else**, because they are cheap,
  they are useful to other people immediately, and they are how a coordination
  effort earns the right to convene one. A registry nobody asked for is a
  smaller failure than a campaign nobody joins.
- **The first five disclosures set the reputation permanently.** Slow and
  correct beats fast and voluminous, and the temptation to scale contact volume
  before trust exists is the single most likely way to destroy this.

---

## 8. The compressed version

Loupe is a good scanner and we just improved it. That was never the problem.

The problem is that **nobody is pointing anything at anything**, and no licence
fixes that, because the cost of scanning falls on the individual while the
benefit falls on the ecosystem. Open source removed the prohibition and left the
economics alone.

What is missing is the boring infrastructure: a public record of what has been
examined and how, shared configurations so nobody inherits one tool's blind
spot alone, a way to notice when two runs disagree, a room to coordinate in,
and somebody paying for compute.

**Our own experiment is the proof.** One configuration missed a bug that cost
594 BTC. Two configurations found it. The entire difference was having a second
opinion — which is exactly what an ecosystem is for, and exactly what does not
currently exist.
