NIP-PRO
=======

Programs
--------

`draft` `optional`

This NIP defines `kind:30182` **programs**: addressable, signed state
machines of named steps with per-step bounds.

A program is a unit of composable software. It is not specific to an agent,
an executor, a model, or a product. A program says **what the steps are and
what bounds them**, and says nothing about who runs them or where.

A program carries no code, no commands, and no prompts. This is the property
everything else rests on: a program is safe to fetch from a stranger,
because the worst a hostile one can do is describe a shape the host
declines.

## Kind

`kind:30182` is in the NIP-33 parameterized replaceable range
(30000–39999) per [NIP-01](../official/01.md): addressed by
`(pubkey, kind, d_tag)`, with only the latest event per address retained.

A dedicated kind is taken rather than NIP-78 `kind:30078`
application-specific data, for the reasons [NIP-AP](../block/NIP-AP.md)
gives: it isolates the address space so program slugs cannot collide with
another application's `d` tag choices, and it lets an indexer recognize a
program from the kind alone.

## Envelope

```jsonc
{
  "kind": 30182,
  "pubkey": "<author pubkey, hex>",
  "created_at": 1789900000,
  "tags": [
    ["d", "delegate-fan-out"],
    ["name", "Fan out one session per task"],
    ["step", "query"],
    ["step", "decide"],
    ["step", "check"],
    ["step", "delegate"]
  ],
  "content": "<json body>"
}
```

There MUST be exactly one `d` tag, matching:

```
^[a-z0-9][a-z0-9_-]{0,63}$
```

The `step` tags list the distinct step kinds the body uses, so a host can
filter for programs it can run **without fetching and parsing every body**.
They duplicate the body and the body is authoritative; a host that finds
them disagreeing refuses the program.

## Body

```jsonc
{
  "v": 1,
  "summary": "Runs one delegated session per task, in parallel.",
  "inputs": {"tasks": "list", "executor": "capability-slug"},
  "outputs": {"results": "list"},
  "steps": [
    {"name": "select",       "kind": "query",    "bounds": {"max_results": 12}},
    {"name": "independence", "kind": "decide",
     "question": "openagents.independence.v1",
     "bounds": {"refuse_below": 0.7, "requires_calibration": true}},
    {"name": "admit",        "kind": "check",
     "bounds": {"refuse_on": "cannot_enforce_intersection"}},
    {"name": "fan_out",      "kind": "delegate",
     "bounds": {"concurrent_max": 6, "isolation": "worktree", "minutes": 60}},
    {"name": "accept",       "kind": "decide",
     "question": "openagents.completion.v1",
     "bounds": {"per_requirement": true}}
  ]
}
```

Step names MUST be unique within a program. A step's output is addressable
by its name.

## Step kinds

| Kind | What it does |
| --- | --- |
| `query` | A structured lookup. Deterministic given its inputs. |
| `check` | A deterministic admission test. Passes or refuses. |
| `decide` | A typed question put to a decision model. |
| `delegate` | Work handed to an executor. |
| `program` | Another program, by address. See [Composition](#composition). |

The registry is open: a future NIP may define more.

**A host that does not recognize a step kind MUST refuse the whole program.**
It MUST NOT skip the step. A program whose unknown steps are skipped is a
different program, and it is the one a host would run by accident.

### `decide` steps name a question, not a question's text

A `decide` step carries a **question identifier**. It MUST NOT carry the
question's wording.

Question text belongs to a separately addressed and separately digested
question set, because rewording a question changes what was asked. A program
that inlined its wording could not say which version produced a result, and
two runs of "the same" program would not be comparable.

## Bounds

Every step carries `bounds`. Bounds are the universal contract between a
program and whatever runs it, and they are the reason a program is safe to
run at all.

A host MUST refuse a step whose bounds it cannot enforce. It MUST NOT run
the step unbounded, and it MUST NOT silently substitute a bound of its own.
An executor that accepts a bound and ignores it is more dangerous than one
that refuses it, which is why a capability manifest under
[NIP-CAP](NIP-CAP.md) states what it *cannot* enforce positively rather than
leaving it to be inferred.

Bounds are not a security boundary on their own. They bound work that is
already permitted; they do not grant permission.

## Composition

A step of kind `program` runs another program:

```jsonc
{"name": "review_each", "kind": "program",
 "program": "naddr1…",
 "bounds": {"concurrent_max": 4}}
```

This is what makes a program a composable unit rather than a script format.
Three rules keep composition from becoming a way to hide behaviour:

1. **Bounds narrow and never widen.** A child program runs under the
   intersection of its own bounds and its parent's. A parent that permits
   four concurrent sessions cannot be made to permit six by a child that
   asks for six.
2. **Depth is bounded.** A host MUST enforce a maximum composition depth and
   MUST refuse a program that exceeds it. The default is 8.
3. **Cycles are refused.** A host MUST detect a program that reaches itself,
   directly or through others, and refuse it rather than bounding it by
   depth alone. Depth alone would run a cycle seven times before stopping.

A host resolves child programs **before running any step**, so a program
that cannot be fully resolved fails before it has done anything.

## Versioning

`v` is the body's schema version. A host that does not know a `v` MUST
refuse the program rather than reading the fields it recognizes.

Programs are replaceable by address, so publishing a new event at the same
`d` tag replaces the program everywhere. An author who wants two versions to
coexist publishes them at two `d` tags. A run SHOULD record the event id of
the program it ran, not only the address, so evidence names the exact bytes.

## What a program is not

- **Not a script.** It carries no code, commands, or prompts.
- **Not a permission.** Resolving a program grants nothing. A host runs a
  program's steps only against capabilities its operator already permits.
- **Not a promise that it will run.** A host that lacks a step kind, an
  executor, or a bound refuses, and refusing is the correct outcome.
- **Not machine-specific.** A program says nothing about where it runs.
  Which executor fills a `delegate` step is resolved at run time from what
  the machine has, which is [NIP-CAP](NIP-CAP.md)'s subject and not this
  one's.

## Trust

Publishing a program says its author wrote it. It says nothing about whether
running it is a good idea.

A host SHOULD resolve programs only from pubkeys its operator names, SHOULD
refuse a `v` it does not know, and MUST ignore fields it does not
understand rather than guessing at them.

Because a program carries no code, the attack surface is the **shape** it
describes: a program that fans out to a thousand delegations, or composes
to a great depth, or names a step kind that a permissive host treats as
optional. The bounds rules above are what make those refusable, and a host
that does not enforce them is where the risk actually lives.
