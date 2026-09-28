# NIP-XP — Quests, acceptance, and experience points

`draft` `optional` — v1, 2026-09-26; the `reproduce` rule added
2026-09-28; the `playtest` rule added 2026-09-28; the `per-awardee`
uniqueness policy and trainer profiles added 2026-09-28. The
[shared contracts](contracts.md) are normative.

This NIP publishes quests, a referee's acceptance of a completed quest, and
the experience points (XP) that acceptance carries, as signed Nostr events.
A reader fetches them from relays, re-checks each acceptance against the
signed evidence it names, and computes XP only from referees it trusts.
`crates/nostr` (`xp`) is the conformance implementation;
`xp_ledger` (`crates/xp-ledger`) derives a ledger and `microcoder xp` publishes and reads
over a relay. `docs/coder/guides/xp.md` is the operator's guide.

XP is evidence of verified accepted work. It is never a balance: it can't
be spent, transferred, sold, or converted into sats, compute credits, or
any permission. It is awarded once per accepted outcome, never per token,
command, commit, run, or unit of time. These are the rules of the
[Verse economy](../../docs/verse/gdd.md#economy) and of the
[Minecraft guild design](../../docs/minecraft/economy.md#xp-and-winning);
this NIP is their transport.

Sats are out of scope. A quest purse that pays sats for an accepted
completion settles through the agent-labor and payment contracts
([NIP-MKT](NIP-MKT.md), [NIP-LAB](NIP-LAB.md), and [NIP-X402](NIP-X402.md);
migration package M18), after acceptance and separately from it. Paying a
purse never creates XP, and XP never unlocks a purse.

## Kinds

These are OpenAgents draft assignments, not upstream registrations.

The playtest report was first drafted as `3195`, which NIP-EVAL already
owns for Gym results publications. It moved to `3197` before any report
was published. The [kind registry](README.md#kind-registry) lists every
OpenAgents kind once.

| Kind | Class | Record |
| --- | --- | --- |
| `30193` | Addressable | One frozen quest version, at its own address. |
| `3193` | Regular | An award: a referee's acceptance of one completion. |
| `3194` | Regular | Irreversible revocation of one award. |
| `1985` | Regular (NIP-32) | Optional achievement label that points at an award. It carries no XP. |
| `3196` | Regular | A moderator's record of a completed playtest session (the `playtest` rule). |
| `3197` | Regular | A tester's content-free playtest report (the `playtest` rule). |
| `13193` | Replaceable | A trainer profile: the key's opt-in to having its level shown, and its other keys. It carries no XP. |

Every XP body (`30193`, `3193`, `3194`, `13193`) is a UTF-8 JSON object with `v: 1`,
`requires` (the empty list in this version), and `type`: `quest`, `award`,
`revocation`, or `profile`. Each event carries exactly one `t` marker
`oa:xp:<type>:v1`. A body whose `type`, marker, and kind disagree is
refused. Unknown body keys, rules, roles, and uniqueness policies are
refused. Every `t` value is lowercase.

`schemas/xp-quest.v1.json`, `schemas/xp-award.v1.json`, and
`schemas/xp-revocation.v1.json` describe the three bodies, and
`schemas/xp-recipe.v1.json` the recipe a `reproduce` quest pins, and
`schemas/xp-playtest-report.v1.json` and
`schemas/xp-playtest-session.v1.json` the two `playtest` records, and
`schemas/xp-profile.v1.json` the trainer profile. The schema
dialect has no `pattern` keyword, so the validator checks the ID grammar,
the hex fields, and the coordinates itself.

## Roles

- A **referee** publishes quests and signs their awards and revocations.
  The referee of a quest is the key that signed it; only that key awards
  it.
- An **author** wrote the [NIP-KB](NIP-KB.md) entry version a completion
  uses.
- A **runner** ran the paired runs and signed the [NIP-EVAL](NIP-EVAL.md)
  evidence (`3189`) that shows the entry helped.
- A **claimant** published an attempt as run evidence (a claim) whose
  recipe a `reproduce` quest pins.
- A **reproducer** reran that recipe and signed run evidence of the rerun
  (a reproduction).
- A **reader** keeps its own list of trusted referees, and optionally of
  trusted runners, and derives XP from them. A reader's runner list also
  lists the reproducers it trusts.

- A **tester** played a build and signed a playtest report.
- A **triager** accepted the tester's contribution on a public issue; for a
  moderated or group session, the triager is the session's **moderator**,
  who signed the session record.

The author and the runner MUST be different keys, and so MUST the claimant
and the reproducer, and the tester and the triager. Evidence an author signs about their own entry, and a
reproduction a claimant signs of their own attempt, never earn XP.

## Quests (`30193`)

A quest is a frozen version: its rule, award, and season never change after
publication. To change anything, the referee publishes a new version at a
new address.

The quest ID matches the NIP-KB entry ID grammar,
`^[a-z][a-z0-9.-]{0,127}$`. The `d` tag is the **address**
`<id>@<version>`, such as `tb4.fix-git.beat-fable-low@1`. Because each
version has its own address, a new version never replaces an old one on a
relay, and awards for the old version stay verifiable.

The body (illustrative values):

```json
{
  "v": 1,
  "requires": [],
  "type": "quest",
  "id": "tb4.fix-git.beat-fable-low",
  "version": 1,
  "season": {"id": "2026-q4", "opens_at": 1790000000, "closes_at": 1798000000},
  "title": "Beat Fable 5.1 low's cheapest winning run on fix-git",
  "objective": "Publish an entry that makes a paired Microcoder run pass fix-git for less than Fable 5.1 low's cheapest winning run, on a task the entry wasn't written from.",
  "acceptance": {
    "rule": "kb-transfer",
    "task": "fix-git",
    "min_pass_rate": 1.0,
    "max_usd_per_run": 0.21
  },
  "reference": {
    "label": "Fable 5.1 low, cheapest winning run",
    "usd": 0.21,
    "seconds": 312,
    "source": "bench/terminal-bench/results/…"
  },
  "award": {"author": 6, "runner": 4},
  "completions": "first"
}
```

| Field | Contract |
| --- | --- |
| `season` | A slug `id` and the Unix-second window `[opens_at, closes_at]`, with `opens_at < closes_at`. Awards and their evidence fall inside it. |
| `title`, `objective` | Display text, at most 200 and 4,000 characters. Never an instruction to an agent. |
| `acceptance` | The rule and its parameters; see [Acceptance rules](#acceptance-rules). Each rule has its own closed set of keys. |
| `reference` | The run the quest is measured against, or `null`: a label, its cost in dollars, its wall time in seconds, and where it's recorded. Display and provenance only; the executable bar is in `acceptance`. |
| `award` | Fixed XP per role, with exactly the rule's roles: `author` and `runner` under `kb-transfer`, `claimant` and `reproducer` under `reproduce`. A role MAY be 0. The quest's award is the sum, at least 1 and at most 1,000. Roles split the award; they never multiply it. |
| `completions` | The uniqueness policy; see [Uniqueness policies](#uniqueness-policies). `first`: the first accepted completion per uniqueness key earns the award, once; the key is the quest version's coordinate, except under `playtest`, whose rule derives it. `per-awardee`: each distinct reproducer earns the award once, up to `max_awards`; only `reproduce` takes it. |
| `max_awards` | Under `per-awardee`, required: the most live awards the quest version pays, 1 to 10,000. Absent under `first`. (A `playtest` quest states its own in `acceptance`.) |

Tags:

| Tag | Count | Value |
| --- | --- | --- |
| `d` | exactly 1 | The address, `<id>@<version>`. |
| `t` | exactly 1 | `oa:xp:quest:v1`. |
| `t` | exactly 1 | `oa:xp:season:<season id>`. |
| `t` | exactly 1 | `oa:xp:rule:<rule>`. |

A quest's coordinate is the NIP-01 address `30193:<referee>:<address>`.

### Rewriting a frozen version

Two different `30193` events from one referee at one address are a rewrite
of a frozen version. A reader that sees both MUST report the conflict and
MUST NOT count any award for that address; it never chooses by timestamp.
A relay keeps only the newest event at an address, so a referee that
rewrites a version also orphans every award that pinned the earlier event:
those awards name a quest event the reader can't find, and don't count.

## Acceptance rules

### `kb-transfer`

A knowledge quest is completed by an entry version that, in paired runs by
someone other than its author, helped on the quest's task, out of sample.
A completion names one `3190` entry event and one `3189` evidence event. It
is accepted when all of these hold:

1. The entry is a valid NIP-KB `3190`, and the evidence is a valid NIP-KB
   evidence `3189` whose report subject is exactly that entry event: its
   `event` names it, its `id` is the entry's qualified ID in the author's
   namespace, and its `artifact` has the digest and size of the entry's
   `document`.
2. The evidence's signer (the runner) isn't the entry's signer (the author).
3. The quest's `task` isn't a task the entry was written from: none of the
   entry document's `provenance.written_from` runs is a run of that task.
   A task an entry was written from never counts as evidence for it.
4. The report's `verdict` is `pass` under its own pinned rule, and its
   paired tasks (`meta.kb.pairs` in this version's evidence) include the
   quest's task with at least one graded run in each arm.
5. On that task, the with-entry arm passes at least `min_pass_rate` of its
   runs and, when `max_usd_per_run` is set, costs strictly less per run.
6. The evidence was published inside the season.

An `inconclusive` historical screening report does not satisfy rule 4.
Publication or a runner's declaration that runs were prospective does not
change that verdict. OpenAgents' current historical evidence producer always
returns `inconclusive`; it cannot complete these quests. A separately verified
prospective report can still satisfy the existing rule. In the v1 pair shape,
`with.usd` is a complete comparable arm total, never a known lower bound;
an unknown total is `null` and cannot satisfy the cost check.

Both the referee, before signing, and every reader, before counting, run
these checks from the signed events alone. Nothing in the rule depends on
the referee's word except the choice to accept.

### `reproduce`

A reproduction quest is completed by an independent rerun of a published
attempt, from its pinned recipe, that the benchmark's grader accepts. It
needs no knowledge entry, so it is the rule a newcomer can complete first,
and it builds the pool of runners that `kb-transfer` needs.

```json
"acceptance": {
  "rule": "reproduce",
  "task": "build-pmars",
  "recipe": "<64 hex: SHA-256 of the recipe's RFC 8785 canonical JSON>",
  "claim": {"id": "<3189 event id>", "pubkey": "<claimant>", "kind": 3189}
},
"award": {"claimant": 0, "reproducer": 50}
```

A **recipe** (`schemas/xp-recipe.v1.json`) pins how an attempt ran. Every
field is a string of 1 to 256 characters:

```json
{
  "v": "openagents.xp-recipe.v1",
  "benchmark": "terminal-bench",
  "benchmark_version": "2.1",
  "task": "build-pmars",
  "image": "alexgshaw/build-pmars:20251031",
  "agent": "microcoder",
  "model": "gpt-6-luna",
  "effort": "medium",
  "knowledge": "off"
}
```

**Run evidence** is a NIP-EVAL `3189` publication in this profile:

- Tags: `t` `oa:eval:v1`, `t` `oa:xp:run:v1`, `x` (the report's digest,
  as in NIP-KB evidence), and one `e` per claim the evidence cites: none
  for a claim, the claim for a reproduction. The `oa:xp:run:v1` marker
  keeps NIP-KB readers from reading run evidence as knowledge evidence.
- The publication's `subject` is a DefinitionRef with no `event`: `id` is
  `<claimant>:recipe/<task slug>` and `artifact` names the recipe's
  canonical bytes, with the schema `openagents.xp-recipe.v1`. A claim and
  its reproductions share one subject.
- `meta.run_report` holds the report's exact bytes, at most 32 KiB. The
  report is `openagents.eval-report.v1` with `evaluator` equal to the
  signer, the same subject, `verdict` `pass` when the record's reward is at
  least 1 and `fail` otherwise, and `meta.run: {recipe, record}`.
- The **record** is a bounded extract of one graded run: `task`, `image`,
  `model`, `effort`, `knowledge`, and `reward`, which MUST match the
  recipe (the reward aside), and `ending`, `steps`, `seconds`, and `usd`,
  each nullable. `summary` is the ArtifactRef of the whole run record file
  (Microcoder's `summary.json`).

A completion names one claim, the one the quest pins, and one
reproduction. It is accepted when all of these hold:

1. The claim is valid run evidence whose recipe has the quest's `recipe`
   digest and `task`, whose subject belongs to the claim's signer, and
   whose record passed.
2. The reproduction is valid run evidence with the claim's subject that
   cites the claim.
3. The reproduction's signer (the reproducer) isn't the claim's signer
   (the claimant).
4. The reproduction's record passed, and its `summary` digest differs from
   the claim's: a rerun has a run record of its own.
5. The reproduction was published inside the season, and not before the
   claim.

Readers run these checks from the signed events alone. Before signing, the
referee also obtains the reproducer's run record file, checks that its
bytes have the digest the reproduction names, and checks that the extract
is the one those bytes give. A reader can't repeat that check without the
file, so, as under `kb-transfer`, a reader that wants more than the
referee's word lists the reproducers it trusts.

### `playtest`

A playtest quest is completed by a contribution to the OpenAgents app's
playtest program ([`docs/game/playtesting.md`](../../docs/game/playtesting.md))
that a triager accepted. Joining, installing, or opening a build is never a
contribution, so no quest exists for them.

```json
"acceptance": {
  "rule": "playtest",
  "contribution": "bug",
  "builds": ["1.0.0 (14)", "1.0.0 (15)"],
  "severities": ["p2", "p3"],
  "max_awards": 200
},
"award": {"tester": 20, "triager": 0}
```

| Field | Contract |
| --- | --- |
| `contribution` | `feedback`, `bug`, `design`, `verified-fix`, `session`, or `diary`. |
| `builds` | The season's build list, 1 to 64 distinct strings such as `1.0.0 (15)`. A report on any other build doesn't count. |
| `severities` | `bug` only, and required: the triage severities (`p0` to `p3`) this quest pays for. |
| `script`, `format` | `session` only, and required: the session script (a slug such as `session-2`) and `unmoderated`, `moderated`, or `group`. |
| `max_awards` | Required: the most live awards this quest version pays, 1 to 10,000. |

The **playtest report** (`3197`) is signed by the tester and holds no
text. The report itself travels privately to the triage key (a NIP-17
message sealed with NIP-44); this event only commits to it:

```json
{"v": 1, "requires": [], "type": "playtest-report", "build": "1.0.0 (15)",
 "platform": "ios", "kind": "bug", "digest": "<64 hex>", "script": null}
```

`platform` is `ios` or `android`; `kind` is `bug`, `confusing`, `idea`,
`felt-good`, `verified`, `session`, or `diary`; `digest` is the lowercase
hex SHA-256 of the private report's exact bytes (the NIP-17 kind `14`
rumor's `content`); `script` names a session script or is `null`. The one
tag is `t` `oa:xp:playtest-report:v1`; the time is `created_at`.

The **session record** (`3196`) is signed by the moderator: `script`,
`format` (`moderated` or `group`), `build`, `tester` (hex), and `held_at`,
with the tags `t` `oa:xp:playtest-session:v1` and one `p`, the tester. A
moderator never records their own session.

A playtest award names the report, then, for a moderated or group
session, the session record, in `evidence`, and lists the tester, then the
triager, in `awardees`. It adds `issue` (`owner/repo#number`, required for
`feedback`, `bug`, `design`, and `verified-fix`, absent otherwise), which
is the public record of the acceptance; `severity` (`bug` only, and one of
the quest's `severities`); and `commit` (`design` only: the 40-hex commit
that shipped the change). A contribution is accepted when all hold:

1. The report is a valid `3197`, its build is in the quest's build list,
   its kind is one the contribution takes (`feedback`: bug, confusing,
   idea, or felt-good; `bug`: bug; `design`: bug, confusing, or idea;
   `verified-fix`: verified; `session`: session; `diary`: diary), and it
   was published inside the season.
2. The tester awardee signed the report. The tester is neither the
   triager nor the award's referee: a referee never awards a key it
   controls.
3. For a session, the report names the quest's script. For a moderated or
   group session, the session record is valid, signed by the triager,
   names the report's signer as tester and the quest's script and format,
   a listed build, and a time inside the season. Any other contribution
   names no session record.
4. The issue, severity, and commit are present exactly when the
   contribution needs them, and `accepted_at` is inside the season and no
   earlier than the evidence.
5. The award's `key` is the one this rule derives:
   - `feedback`, `bug`, `design`: `playtest:<season>:report:<issue>`, so one
     issue earns one report-class award from a referee, whichever of those
     quests pays it (a later duplicate report earns nothing);
   - `verified-fix`: `playtest:<season>:verified:<issue>`;
   - `session`, `diary`: `playtest:<season>:<quest address>:<tester>`, once
     per tester per script (one quest version per script) per season.
   The `a` tag is still the quest coordinate, so `#a` finds a quest's
   awards.
6. The quest version has at most `max_awards` live awards from its
   referee. More is the referee overissuing: a reader reports the conflict
   and counts none of that version's awards until the referee revokes the
   extras. It never chooses by timestamp.

A reader can re-check all of that from signed events. It can't re-check
whether the bug was real or how severe it was: that is the referee's
judgment, recorded on the public issue. `playtest` is a weaker rule than
`kb-transfer` and `reproduce`, which is why playtest awards are signed by a
separate **playtest referee** key, and why a client MUST NOT sum playtest
XP into a trainer level: it shows playtest XP beside the trainer level,
never inside it. Playtest titles (`playtester`, `founding-playtester`,
`bug-hunter`, `fix-verifier`, `raider`) are NIP-32 achievement labels on
counted playtest awards, signed by the playtest referee.

A revocation of a playtest award carries the award's `key` and, in its
`a` tag, the quest coordinate.

This rule's uniqueness is rule-derived; the general `per-awardee` policy
the leveling spec proposes (issue #9894) may later subsume it.

Rules are closed: a reader refuses a quest whose rule it doesn't implement.
A future rule, such as a Gym trial with a pinned suite or a coding quest
with an integrator, needs its own rule name and roles.

## Awards (`3193`)

An award is the referee's signed acceptance of one completion. It binds
everything a reader needs to re-check it:

```json
{
  "v": 1,
  "requires": [],
  "type": "award",
  "quest": {
    "id": "<30193 event id>",
    "pubkey": "<referee>",
    "kind": 30193,
    "coordinate": "30193:<referee>:tb4.fix-git.beat-fable-low@1"
  },
  "key": "30193:<referee>:tb4.fix-git.beat-fable-low@1",
  "accepted_at": 1790500000,
  "entry": {"id": "<3190 event id>", "pubkey": "<author>", "kind": 3190},
  "entry_version": {"id": "git.reflog-recovery", "version": 2, "digest": "<64 hex>"},
  "evidence": [{"id": "<3189 event id>", "pubkey": "<runner>", "kind": 3189}],
  "awardees": [
    {"role": "author", "pubkey": "<author>", "xp": 6},
    {"role": "runner", "pubkey": "<runner>", "xp": 4}
  ]
}
```

- The signer MUST equal `quest.pubkey`: a referee awards only its own
  quests. `quest.id` pins the exact quest event, never only its address.
- `key` is the **uniqueness key**. Under `completions: first` it equals the
  quest version's coordinate, so it names the quest version and nothing
  else. Under `per-awardee` it is the coordinate, a colon, and the
  reproducer's public key, `30193:<referee>:<address>:<reproducer>`.
- `entry_version` repeats the entry's ID, version, and document digest (the
  `3190`'s `x` tag) so a reader can match the award without parsing the
  document.
- `evidence` lists exactly one event under `kb-transfer`: the runner's.
- `awardees` lists the author, then the runner. The author's key signed
  the entry, the runner's key signed the evidence, and the two differ.
  Each `xp` equals the quest's `award` for that role.
- `accepted_at` falls inside the season and is no earlier than the
  evidence event's `created_at`.

A `reproduce` award has no `entry` or `entry_version`. Its `evidence`
lists the claim, then the reproduction, and its `awardees` list the
claimant, then the reproducer, each the signer of the event its role
names. A role whose XP is 0 is still listed. The first awardee's role
tells a reader which shape to expect; `schemas/xp-award.v1.json` has both.

Tags:

| Tag | Count | Value |
| --- | --- | --- |
| `t` | exactly 1 | `oa:xp:award:v1`. |
| `a` | exactly 1 | The quest coordinate: equal to `key` under `first`, and its prefix under `per-awardee`. |
| `e` | 3 | The quest, entry, and evidence event IDs; under `reproduce`, the quest, claim, and reproduction. |
| `p` | 2 | The awardees' public keys. |

The `a` tag lets a reader or referee find every award for a quest version
with an `#a` filter, and the `p` tags let a player find their own awards
with `#p`.

### One award per quest version

A referee MUST keep at most one live award per uniqueness key. A reader
that holds two or more unrevoked awards from one referee with the same key
MUST report the conflict and count none of them until the referee revokes
the extras. It never picks one by timestamp.

The award is fixed per quest version, on its first accepted completion. The
following never multiply it:

- Replaying the same evidence, or publishing more evidence for the same
  entry: the key is the quest version, not the evidence.
- Republishing the award: the same event ID counts once, from any number
  of relays.
- More runs, commits, tokens, or time: the rule reads a verdict and a
  paired task, and the award is a constant from the quest.
- More roles: the roles split the fixed total.

A referee that wants a harder or repeated challenge publishes a new quest
version, such as one whose bar is the current best run.

### Uniqueness policies

A quest's `completions` names how many completions it pays:

| Policy | Uniqueness key | Pays | Rules |
| --- | --- | --- | --- |
| `first` | The quest version's coordinate (`playtest` derives its own) | The first accepted completion, once | All |
| `per-awardee` | `<coordinate>:<reproducer>` | Each distinct reproducer once, and at most `max_awards` awards | `reproduce` |

`per-awardee` is for tutorials and dailies: every newcomer can complete the
same quest version once. Its rule names a keyed role, the reproducer, and
every other role's XP MUST be 0, so the claimant that every completion
shares is never credited once per reproducer. A reader that holds more
live awards from the quest's referee on one quest version than its
`max_awards` MUST report the conflict and count none of them until the
referee revokes the extras, as for two awards on one key. A repeated
reproducer is two live awards on one key, a conflict. A Sybil farm can
take at most the quest's small award per key, and each key still needs a
passing run of its own that the referee checked.

## Revocations (`3194`)

A revocation ends one award:

```json
{
  "v": 1,
  "requires": [],
  "type": "revocation",
  "award": {"id": "<3193 event id>", "pubkey": "<referee>", "kind": 3193},
  "key": "30193:<referee>:tb4.fix-git.beat-fable-low@1",
  "reason": "The runs were graded against the wrong verifier version."
}
```

Tags: exactly one `e` naming `award.id`, exactly one `a` naming the quest
coordinate (equal to `key` under `first`), and
the marker `oa:xp:revocation:v1`. The signer MUST equal `award.pubkey`: only
the award's referee revokes it. `reason` is display text of 1 to 1,000
characters.

A revocation is irreversible for that award. It frees the uniqueness key:
the referee MAY then award the quest version to a correct completion, with
an `accepted_at` later than the revoked award's. A reader keeps revocations
so a changed ledger can be explained. Another key's disagreement with an
award is not a revocation; a reader acts on it by changing its trust.

## Achievements and NIP-32

Achievements, such as "first out-of-sample transfer" or "beat Fable on
fix-git", are [NIP-32](../official/32.md) labels, as Voyager already
publishes for completed quests. Under this NIP an achievement label is a
pointer to an award:

| Tag | Value |
| --- | --- |
| `L` | `openagents.xp` |
| `l` | A slug such as `beat-reference` or `first-transfer`, with the namespace `openagents.xp` as its mark. |
| `e` | Exactly one: the `3193` award it celebrates. |
| `p` | Each awardee of that award. |

A reader shows an achievement label only when its signer is the award's
referee and the award counts in that reader's ledger. A revoked or refused
award takes its labels with it.

Labels alone can't carry XP, for these reasons:

- **No binding.** A `1985` label names a target and a string. It has no
  field that pins the exact quest version, entry version, evidence, or
  fixed amount, so a reader can't re-check what the label claims.
- **No uniqueness.** Nothing stops a labeler from labeling the same
  achievement many times. XP needs a key that can pay at most once.
- **No amount rule.** A label has no schema for an amount or its split
  among contributors, so each client would invent one.
- **No reliable retraction.** Undoing a label means a NIP-09 deletion,
  which relays may ignore and which leaves no reason. XP needs an
  irreversible, signed, explainable revocation.
- **No roles.** Anyone can label anyone. Acceptance needs the quest's own
  referee, a runner who isn't the author, and a rule a reader can run.

So numeric XP lives in awards, and labels stay what NIP-32 is good at: a
visible, filterable name for something that already counts.

## Deriving a ledger

A reader derives XP from signed events every time; it never stores a
balance it can't re-derive. With its trusted referees `R` and, optionally,
its trusted runners `U`:

1. Fetch `30193`, `3193`, and `3194` events authored by `R`, then the `3190`
   entries and `3189` evidence (including claims and reproductions) the
   awards name, by exact event ID. Events from several relays are merged by
   event ID.
2. Validate every quest. Report addresses with two different quest events,
   and count no award under them.
3. Validate every revocation, keeping those whose signer is the award's
   referee.
4. For each award signed by a key in `R`: skip it when revoked; otherwise
   validate it alone, bind it to the exact quest event it names, and run
   the quest's rule over the exact entry and evidence events it names. A
   missing event means the award isn't counted, never that it is assumed
   valid. When `U` is non-empty, the runner (under `reproduce`, the
   reproducer) MUST be in `U`.
5. Group the surviving awards by referee and uniqueness key. A group with
   more than one award is a conflict and counts for no one. A quest
   version whose `max_awards` (under `per-awardee` or `playtest`) is
   below its number of surviving awards is a conflict too, and none of
   its awards count.
6. An awardee whose `xp` is 0 gets no credit.
7. For each remaining award, credit each awardee its `xp`.

The result is XP per public key, with the awards behind each credit, the
revoked awards, the refusals, and the conflicts. Two readers with different
trust lists get different ledgers from the same relay, and both are right
for their readers.

Levels, stat points, titles, and grants are the client's reading of a
ledger, not part of this NIP. A client MAY show levels from XP; it MUST NOT
turn XP into spending authority, wider tool access, file-system scope, or
disclosure of private evidence.

## Trainer profiles (`13193`)

XP is public by construction: anyone can derive any key's ledger from the
awards. A trainer profile doesn't change what counts. It is the key's
opt-in to being advertised: a client shows a level on a name tag or a rank
board only for a key whose newest valid profile says `shown: true`, and a
key that never published one, or whose newest profile says `false`, gets
no level there. The ledger stays computable for every key, and a trainer
still sees its own XP.

```json
{
  "v": 1,
  "requires": [],
  "type": "profile",
  "shown": true,
  "keys": ["<hex public key>"]
}
```

| Field | Contract |
| --- | --- |
| `shown` | Whether clients may show this trainer's level on name tags and boards. |
| `keys` | The trainer's other keys, at most 16, each 64 lowercase hex characters, distinct, and never the signer. A listed key counts toward the trainer only when it signs a matching link back; a profile alone never claims a key. |

Tags: the marker `oa:xp:profile:v1` and one `p` per listed key, in the
order of `keys`. The event is replaceable: a reader takes the author's
newest valid profile, by `created_at` and then the lowest event ID, as
NIP-01 orders replaceable events. To hide its level, a trainer publishes a
newer profile with `shown: false`. A profile carries no XP and no level;
a reader never reads a level or a total from it.

## Trust

Trust is per reader, as in [NIP-KB](NIP-KB.md#trust). A reader keeps its
own list of referees whose awards count and, optionally, of runners whose
evidence counts. A reader MAY trust its own referee key. There is no global
ledger and no global leaderboard: a board is a reader's view of the
referees it trusts.

Two referees' quests are different quests, even when they name the same
task. A reader that trusts both counts both; a reader that wants one board
trusts one referee.

Relays are transport. A relay can drop, delay, or replay events, but it
can't forge one, and every check above uses the signed events, never the
subscription they arrived on.

## Abuse

| Attack | What stops it |
| --- | --- |
| **Self-evidence**: an author runs and signs evidence for their own entry. | The runner MUST differ from the author, checked by the referee and by every reader. |
| **Self-reproduction**: a claimant reproduces their own attempt. | The reproducer MUST differ from the claimant. |
| **Copied reproduction**: a key republishes the claim's run record as its own rerun. | The reproduction's `summary` digest MUST differ from the claim's, and the referee checks the reproducer's own file before signing. |
| **Fabricated reproduction**: a key signs a passing extract of a run it never made. | The referee accepts a reproduction only after checking the run record file it names; a reader MAY list trusted reproducers. As with Sybil runners, keys can't be tied to people. |
| **Sybil runners**: an author makes a second key to run their evidence. | Keys can't be tied to people, so the protocol can't detect this alone. The referee accepts only evidence it can re-derive from retained runs or that comes from runners it trusts, and a reader MAY list trusted runners so evidence from any other key never counts. |
| **In-sample evidence**: an entry written from the quest's task "helps" on it. | The rule reads the entry document's `provenance.written_from` and refuses a quest task the entry was written from. |
| **Cherry-picked runs**: reporting only the winning run. | The rule reads the paired task's whole arm: every graded run counts toward the pass rate and the cost per run, and the report's own verdict must pass. |
| **Replay and farming**: republishing awards, evidence, or runs. | Awards count once per event ID, and at most one award per uniqueness key counts. The award is a quest constant. |
| **Inflation**: an award with more XP than the quest states. | Readers bind each award to the exact quest event and refuse any XP that differs from its table. The quest's total is capped at 1,000. |
| **Rewriting a quest**: changing a frozen version's bar after the fact. | A second event at one address is a conflict, and awards bind the exact quest event ID. |
| **Referee equivocation**: two live awards for one quest version. | The key counts for no one until the referee revokes the extras. |
| **Referee compromise**: a stolen referee key signs awards. | The referee revokes what it can while it still holds the key. Every reader removes the key from its trust list, which drops all of that key's awards at once. A reader that retained the award IDs it counted before the compromise can keep those; timestamps alone don't help, because an attacker can backdate `created_at` and `accepted_at`. |
| **Advertising a key that didn't ask**: showing a stranger's level over their head. | Clients show levels on tags and boards only for keys whose newest profile says `shown: true`, signed by that key. |
| **Colluding referee and runner.** | Per-reader trust is the remedy: a reader trusts referees whose acceptances it can audit, and every award names the evidence needed to audit it. |

## Validation

A reader accepts a `3193` only when all of these hold:

1. The event ID and signature verify (NIP-01).
2. The body parses with the [shared encoding rules](contracts.md#encoding-and-validation),
   has `v: 1`, an empty `requires`, `type: award`, and no unknown keys.
3. The signer is the quest's referee, `key` is the one the quest's
   uniqueness policy gives (the quest coordinate under `first`; the
   coordinate and the reproducer under `per-awardee`; the rule's key under
   `playtest`), and the `a`, `e`, and `p` tags agree with the body.
4. The awardees are the author, then the runner, under `kb-transfer`, the
   claimant, then the reproducer, under `reproduce`, or the tester, then
   the triager, under `playtest`; each signed the event its role names,
   and the two differ.
5. The exact quest event it names is available and valid, the award falls
   inside the season, and each role's XP equals the quest's table.
6. The exact events it names are available and valid, the entry version
   matches `entry_version` under `kb-transfer`, the claim is the one the
   quest pins under `reproduce`, and the quest's rule accepts them.
7. No trusted revocation names it, and no other live award from its referee
   shares its key.

Quests and revocations are checked the same way against their own fields.

## Relay and client conformance

Relays store `3193` and `3194` as regular events and `30193` as
addressable, validate the signature, and apply the lowercase `t` rule. A
relay doesn't evaluate rules, compute XP, or rank players. Removal from one
relay is neither revocation nor global erasure.

Clients cover, with fixtures: a tampered quest (signature), a quest whose
tags disagree with its body, an unknown rule or uniqueness policy, an award
signed by someone other than the quest's referee, an award whose XP differs
from its quest, self-evidence, in-sample evidence, a report that doesn't
pass or doesn't pair on the task, evidence outside the season, a report
that cites the entry event but measured other document bytes or names the
entry outside its author's namespace, a revoked
award and its replacement, two live awards for one key, a rewritten quest
version, an award whose evidence is unavailable, and a revocation signed by
someone other than the award's referee. For `reproduce`: a claimant's own
reproduction, a failed reproduction, a copied run record, a reproduction
outside the season, one that doesn't cite the claim, a claim the quest
doesn't pin, a claim that didn't pass, tampered run evidence, a record
that didn't follow its recipe, swapped roles, inflated XP, and a
reproduction whose record file the referee doesn't have. For `playtest`: a build outside the list, a report kind the contribution
doesn't take, a report outside the season, an unpaid severity, a missing
issue or severity, the tester as triager or referee, a session without its
record or with another tester's or the tester's own, a report on another
script, a key the rule doesn't derive, one issue paid by two quests, and a
quest version over its `max_awards`. For `per-awardee`: a missing or
out-of-range `max_awards`, `max_awards` on a `first` quest, a claimant
share, a rule without a keyed role, a key that names the quest version or
another key instead of the reproducer, a `first` award keyed to a
reproducer, one reproducer paid twice, a quest version over its
`max_awards`, and a revocation of a keyed award.
`crates/nostr/src/xp/tests.rs`, `crates/nostr/src/xp/reproduce/tests.rs`,
`crates/nostr/src/xp/playtest/tests.rs`,
`crates/xp-ledger/src/tests.rs`, and
`crates/microcoder/src/xpnet/tests.rs` hold them.

Advertise `nip-xp-v1` in NIP-11 `supported_extensions` only for a relay
role covered by those fixtures. Keep the draft name out of numeric
`supported_nips`.
