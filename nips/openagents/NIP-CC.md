NIP-CC
======

Coder Capabilities and Programs
-------------------------------

`draft` `optional`

This NIP defines three addressable, signed documents: `kind:30180`
capability manifests, which say **how to drive an executor**; `kind:30181`
operator policies, which say **which executors an operator prefers**; and
`kind:30182` programs, which are **the reusable, composable unit of work**
the decision engine selects.

## Capabilities and programs

A **capability** is something you can reach. A **program** is something you
can run. They are separate kinds because they change for different reasons
and at different rates.

| | Capability | Program |
| --- | --- | --- |
| Answers | What is available, and what will it promise? | What are the steps, and what bounds them? |
| Belongs to | the machine and the executor | the work |
| Changes when | you install, upgrade, or lose an executor | you decide to do the work differently |
| Portable? | No. Presence is per machine. | Yes. A program says nothing about where it runs. |
| Names | `devin-local`, `codex`, `coder-cloud` | `delegate-fan-out`, `review-changes` |

The two are many-to-many. One program reaches several capabilities, and one
capability serves many programs. Installing the Devin CLI does not tell you
what to do with it, and writing `delegate-fan-out` does not require Devin —
the program names a step of kind `delegate`, and which executor fills it is
resolved at run time from what the machine has and what the operator
prefers.

The join between them is bounds. A program's step states the bounds it needs
and a capability manifest states the bounds its executor will keep, so a
host can refuse a pairing before it runs anything. That is the whole reason
`cannot_enforce` is stated positively.

One way to keep them apart: **a capability can be absent, and a program
cannot be wrong about the machine.** If `devin` is not installed, the
capability is simply not an option. The program is unchanged, and runs with
whatever executor is.

## The taxonomy

The taxonomy is deliberate, and it is the glossary's rather than this
document's. A **capability** is a granted ability. An **executor** is the
implementation that performs an agent session, reached through an executor
adapter. A **program** is a state machine with named steps and per-step
bounds, which the decision engine picks at the start of a run.

A **plugin** is none of these. In the reference implementation a plugin is a
sandboxed WebAssembly guest: small, pure, deterministic, and denied the
network. An executor that spawns a process and reaches the internet is the
tier a plugin host refuses to load. The reusable component this NIP is for
is the **program**, not a plugin, and the distinction is load-bearing rather
than cosmetic — see [`docs/programs.md`](../../docs/programs.md).

A capability is a thing a Coder instance can hand work to — another agent's
CLI on the same computer, a cloud lane, a subprocess with a protocol. The
manifest says what the executor is, which bounds it can hold to, and what it
needs present. The policy says what the operator wants done with it.

Neither event says whether the executor is **installed here**. That is
local fact, it belongs on the machine, and publishing it would be an
inventory of somebody's computer with no consumer. See
[Local presence is not an event](#local-presence-is-not-an-event).

## Kinds

Both kinds are in the NIP-33 parameterized replaceable range
(30000–39999) per [NIP-01](../official/01.md): addressed by
`(pubkey, kind, d_tag)`, with only the latest event per address retained.

| Kind | Name | Author |
| --- | --- | --- |
| `30180` | Capability manifest | anyone; typically the capability's maintainer |
| `30181` | Operator capability policy | the operator |
| `30182` | Program | anyone; the composable unit |

A dedicated kind is taken rather than NIP-78 `kind:30078`
application-specific data, for the reasons [NIP-AP](../block/NIP-AP.md)
gives: it isolates this address space so capability slugs cannot collide
with another application's `d` tag choices, and it lets an indexer
recognize a manifest from the kind alone.

### Relationship to NIP-89

[NIP-89](../official/89.md) `kind:31990` declares that an application
handles an event kind. This is the same instinct one layer down: a
capability manifest declares that an executor handles a **task**, and what
it will and will not promise while doing so. The two do not overlap —
nothing here handles an event kind — and a client that understands NIP-89
learns nothing useful from a `30180`.

### Relationship to NIP-AP

[NIP-AP](../block/NIP-AP.md) `kind:30175` describes **how to instantiate an
agent**: identity, system prompt, model, runtime. A capability manifest
describes **how to hand work to an executor that already exists**. A persona
is a blueprint for something we run; a capability is an interface to
something somebody else runs. A manifest MAY name a persona in `persona`
when the executor is a Coder agent instantiated from one.

## Roles

- **maintainer** — publishes and updates a `30180` for an executor. Has no
  authority over anyone's machine; a manifest is a description, not a grant.
- **operator** — publishes a `30181` saying which capabilities to prefer and
  under what bounds. Signed by the operator, so the policy travels between
  that operator's machines without being configuration on each one.
- **host** — the Coder instance that reads both, probes local presence, and
  decides. **The host is the only party that decides anything.** A manifest
  it has not resolved, and a policy it did not fetch for its own operator,
  change nothing.

## Slugs

The `d` tag is the plaintext capability slug, matching the NIP-AP grammar:

```
^[a-z0-9][a-z0-9_-]{0,63}$
```

Plaintext for the same reason NIP-AP gives: manifests are public
definitions meant for discovery and human-readable addressing. A policy's
`d` tag is a policy name, under the same grammar, so an operator can hold
more than one.

## Capability manifest — kind `30180`

```jsonc
{
  "kind": 30180,
  "pubkey": "<maintainer pubkey, hex>",
  "created_at": 1789900000,
  "tags": [
    ["d", "devin-local"],
    ["name", "Devin CLI, this computer"],
    ["transport", "acp"]
  ],
  "content": "<json body>"
}
```

There MUST be exactly one `d` tag. The `transport` tag duplicates
`content.transport` so a relay query can filter without reading bodies.

### Body

```jsonc
{
  "v": 1,
  "summary": "Hands a bounded task to the Devin CLI on this computer.",
  "transport": "acp",
  "detect": {
    "binary": "devin",
    "version": ["devin", "--version"],
    "probe": ["devin", "acp", "--help"]
  },
  "enforces": ["max_turns", "minutes", "model"],
  "cannot_enforce": ["tool_set", "role", "budget_cents", "effort"],
  "sees_repository": true,
  "concurrent_max": null,
  "cost": "operator_account",
  "isolation": ["worktree", "directory"]
}
```

| Field | Meaning |
| --- | --- |
| `transport` | How the host speaks to it. `acp`, `subprocess`, `http`. |
| `detect` | What a host runs to decide the executor is present, and to read its version. Arguments are a fixed argv, never a shell string. |
| `enforces` | Bounds the executor will hold to if given. |
| `cannot_enforce` | **Bounds it will silently ignore.** Load-bearing — see below. |
| `sees_repository` | Whether the executor can read the caller's working directory. A cloud lane cannot. |
| `concurrent_max` | The most simultaneous instances the manifest claims are safe, or `null` for unstated. |
| `cost` | Who pays: `operator_account`, `metered`, `local`. Never a number; prices go stale in a signed event. |
| `isolation` | Which checkout shapes it accepts. |

### `cannot_enforce` is the field that matters

An executor that ignores a bound is more dangerous than one that refuses
it. A host MUST refuse a delegation whose requirements intersect
`cannot_enforce`, rather than issuing it and hoping.

This is the one part of a manifest a host treats as a **constraint** rather
than as advice, and it is why the field is stated positively instead of
being inferred from the absence of an `enforces` entry: an omission is
ambiguous and a refusal must not rest on an omission.

## Operator capability policy — kind `30181`

```jsonc
{
  "kind": 30181,
  "pubkey": "<operator pubkey, hex>",
  "tags": [["d", "default"]],
  "content": "<json body>"
}
```

```jsonc
{
  "v": 1,
  "prefer": [
    {"capability": "devin-local", "weight": 3, "when": "available"},
    {"capability": "coder", "weight": 1}
  ],
  "fan_out_max": 6,
  "require_independence_check": true,
  "never": [
    {"capability": "devin-cloud", "reason": "does not see the checkout"}
  ]
}
```

`prefer` is an ordering, not a rule: it breaks ties among capabilities that
are present and admissible. **A preference never admits a capability that
`cannot_enforce` refused**, and a host that lets one do so has a bug rather
than a configured behaviour.

`fan_out_max` bounds how many delegations one request may start.
`require_independence_check` says the host must not fan out until it has
decided the tasks do not collide — see
[`docs/capabilities.md`](../../docs/capabilities.md).

## Program — kind `30182`

A program is a state machine with named steps and per-step bounds. It is
the unit that composes, the unit an operator shares, and the unit the
decision engine selects at the start of a run.

```jsonc
{
  "kind": 30182,
  "pubkey": "<author pubkey, hex>",
  "tags": [["d", "delegate-fan-out"], ["name", "Fan out one issue per executor"]],
  "content": "<json body>"
}
```

```jsonc
{
  "v": 1,
  "summary": "Takes N issues and runs one delegated session per issue.",
  "inputs": {"issues": "list", "executor": "capability-slug"},
  "steps": [
    {
      "name": "select",
      "kind": "query",
      "bounds": {"max_results": 12}
    },
    {
      "name": "independence",
      "kind": "decide",
      "question": "openagents.coder.independence.v1",
      "bounds": {"refuse_below": 0.7, "requires_calibration": true}
    },
    {
      "name": "admit",
      "kind": "check",
      "bounds": {"refuse_on": "cannot_enforce_intersection"}
    },
    {
      "name": "fan_out",
      "kind": "delegate",
      "bounds": {"concurrent_max": 6, "isolation": "worktree", "minutes": 60}
    },
    {
      "name": "accept",
      "kind": "decide",
      "question": "openagents.coder.completion.v1",
      "bounds": {"per_requirement": true}
    }
  ]
}
```

A step's `kind` is one of `query` (a structured lookup), `decide` (a typed
question put to a decision model), `check` (a deterministic admission test),
or `delegate` (work handed to an executor).

**Every step carries bounds, and a step whose bounds a chosen executor
lists in `cannot_enforce` does not run.** That is the join between this kind
and `30180`, and it is the whole reason a manifest states its refusals
positively.

A `decide` step names a **question identifier**, not question text. The text
belongs to a question set with its own digest, because rewording a question
changes what was asked and a program that inlined its wording could not say
which version produced a result.

### A program is not a script

It names steps and bounds; it does not carry commands, prompts, or code. A
host that cannot resolve a step's `kind` refuses the program rather than
skipping the step. A program is therefore safe to fetch from a stranger in
a way a script is not — the worst a malicious program can do is describe a
shape the host declines.

## Local presence is not an event

A host knows `devin` is installed by running `detect`. That fact stays on
the machine.

Publishing it would say "this computer has these tools at these versions"
to anyone subscribing — a fingerprint of a person's machine — and no party
in this NIP needs it. A fleet that genuinely needs a capability inventory
should publish a **deliberate** one to a **known** audience under its own
kind, and that is not this NIP.

## Trust

A manifest is **not** an authority to run anything. Signing it says a
maintainer wrote it, not that it is safe. A host SHOULD:

- resolve manifests only from pubkeys the operator names;
- treat `detect` as an argv it runs, never a shell string it interpolates —
  a manifest is untrusted input and a manifest that could inject a shell
  command would be a remote execution primitive;
- ignore any field it does not understand, and refuse a `v` it does not
  know rather than guessing;
- re-probe presence rather than trusting a cached answer across an upgrade.

There are no signatures on executables here and no attestation that a
binary named `devin` is Devin. Nothing in this NIP defends against a
hostile local binary, and it should not pretend to.
