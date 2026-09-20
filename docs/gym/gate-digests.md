# Gate digests

A gate in `crates/gym/gates/` answers one product question, and its digest
is the name that answer is recorded under. Every verdict and every row
that the gate judges carries `gate:<sha256>` of it, so a reader can tell
which rule produced which outcome — and can tell a rule from a different
rule wearing the same id.

The digest does not hash the file. It hashes a typed projection of it:
the parts that decide, and nothing that only explains.

## What the digest covers

The projection is a schema of its own,
`openagents.gym.gate-identity.v1`, and it carries:

- the gate's `schema` and `id`, and the identity schema itself, so the
  digest records which encoding produced it;
- the rule's `decides` tag, so a decision rule and a probability rule can
  never collide;
- every bound's `value` and `basis`, so a new threshold or a relabelled
  one is a new rule;
- every bound's `evidence`, the named records the number rests on;
- the enums that say what a statistic covers, such as `variance_basis`
  and `gated_percentile`;
- a pending measurement's `quantity` and `issue`, so filling a recorded
  gap — or recording a different one — is a different rule;
- an optional sub-rule's bounds, such as a probability rule's
  `confident_error_floor`, projected only when the rule carries it. A
  rule without the field projects exactly as it did before the field
  existed, so adding a floor to a new gate does not move an old one.

## What the digest leaves out

Prose stays outside. The `question` line, every bound's `why`, a pending
measurement's `why`, the `$comment`, and `previously` are all for the
reader. Editing a sentence — fixing a typo, moving the document a `why`
cites — changes what the file says to a person, not what the rule does to
a comparison. A sentence that explains a threshold is not the threshold,
and rewording one must not orphan the rows the rule already scored.

This is a projection over typed fields, not a filter over strings.
Identifiers and references are semantic — an issue number inside
`pending_measurement.issue` names the measurement a rule waits on — so
they live in fields whose contents are identity, rather than being
guessed out of prose.

## Evidence names records, not files

A bound's `evidence` cites what its number rests on: a measurement
record such as `2026-09-19-suite-v2-scores`, an issue such as
`openagents#9370`, or a code item such as `gym::calibrate::Map::fit_auto`.
Each id is a name, not a path. The record survives its documentation
moving; `docs/gym/measurements/2026-09-19-suite-v2-scores.md` would not.
The validator refuses an evidence id that contains a `/` or ends in
`.md`, so a path cannot become a rule's identity by accident.

## The v1-to-v2 transition

`openagents.gym.gate.v1` hashed the whole document, prose included, so a
reworded explanation re-identified the rule and orphaned every verdict
recorded under the old digest. `v2` hashes the projection.

That is a real identity transition, and the old identities are kept. A
`previously` entry is not a bare string; it is a binding with three parts:

- `digest` — the value the earlier encoding recorded.
- `equivalent` — the identity projection that digest was reviewed equal
  to, committed as a JSON document. This is the reviewed snapshot: a
  reader can diff it against `rule` and see exactly which policy the
  recording belongs to.
- `recorded_in` — the committed records that pin the digest, so the claim
  it was recorded is checkable. A digest-shaped string is not proof a
  digest was ever recorded; a record that carries it is.

`Gate::has_digest` attributes a recorded digest when it equals the current
digest, or when a `previously` entry names it **and** the entry's
`equivalent` still matches the identity the live rule projects. The second
clause is the point: a gate whose policy moved — a threshold, a basis, an
evidence set, a pending identity — no longer projects to the reviewed
snapshot, so the alias stops attributing even though the digest string is
untouched. That holds whether the edit came through the file or through a
caller mutating a loaded `Gate`. Validation runs the same check at load, so
a file whose `rule` has drifted from a stored `equivalent` refuses to load
rather than silently inherit the old identity.

Nothing regenerates `equivalent`. It is a snapshot of what a reviewer
checked, and a policy change does not earn a new binding by editing — the
honest move is a new versioned gate, or a fresh binding written by someone
who reviewed the equivalence.

The one migration on record:

- `probability-v1` carried the only digest the v1 encoding recorded in
  the result store:
  `gate:368cefd18f308119008db3099c8415af380c9d096d4cbcb6997196bfc6d82013`.
  Its `previously` binds that digest to this file's v2 identity and names
  the records that carry it — the eight result files under
  `crates/gym/results/` and the six calibration records under
  `crates/lev/calibration/`.
- `decision-v1` and `deployment-v1` recorded no digest under v1, so their
  `previously` is empty and nothing written before v2 attributes to them.

Two versioned successors are on record. `deployment-v1` carries
`latency_block_sigma_relative` as `unmeasured`, so every latency criterion
under it is `unverifiable`. `deployment-v2` carries the number once it was
measured on a quiet CPU-only host, 0.17 over 16 blocks per local door
([`measurements/2026-09-20-kev-quiet-latency.md`](measurements/2026-09-20-kev-quiet-latency.md)),
and keeps a `pending_measurement` for the doors that answer in seconds,
which that sweep did not reach. `v1` keeps its file and its digest.

The other: `probability-v1` compares the two
confident-error counts directly, and openagents#9401 showed that refuses an
unchanged door on one direction of every pair of seed blocks whose counts
differ. `probability-v2` is the same rule with a measured
`confident_error_floor`: the count's seed spread, the items it was
measured on, and how many sigmas of the difference a rise may use. `v1`
keeps its file and its digest because recorded verdicts name it; the
suites under `crates/gym/suites/` and the `gym` binary's default now name
`v2`.

The old value stays an alias — the current digest is the v2 encoding's
output, and `previously` never renames it. Archived rows, measurements,
goldens, and calibration records are not rewritten: the store keeps what
the v1 encoding recorded, and the gate remembers what that value was
reviewed equal to.

## What re-judgment means

Re-judgment replays recorded measurements against a rule. It can answer
questions like "would decision-v1's verdict on these rows have differed
under a two-sigma bar?" by running `judge` over the stored scores and
comparing the outcomes.

It cannot do more than that:

- A re-judged verdict is the new rule's opinion of old measurements, not
  evidence the new rule produced them. The rows were scored under the
  digest they carry, and that stays true.
- A changed policy is a changed rule, not the same rule improved. If the
  projection changes, the digest changes, and the honest record is a new
  gate — `decision-v2`, say — not a quiet edit of `decision-v1`.
- Rewording prose, fixing a citation, or moving a document changes
  nothing the digest sees, and needs no new gate.

The version in a gate's id is the contract: same rule, same id, same
digest; different rule, different name for it.
