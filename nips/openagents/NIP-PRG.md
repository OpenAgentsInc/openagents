NIP-PRG
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
    ["step", "delegate"],
    ["step", "module"]
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
| `module` | A WebAssembly module, by content hash. See [Modules](#modules). |

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

## Modules

Some programs need code. A step of kind `module` runs a WebAssembly module:

```jsonc
{"name": "extract", "kind": "module",
 "module": {
   "hash": "sha256:9f2b…",
   "interface": {"entry": "handle", "input": "json", "output": "json"},
   "sources": [
     {"url": "https://example.org/extract-1.4.0.wasm"},
     {"naddr": "naddr1…"}
   ]
 },
 "bounds": {"memory_mib": 64, "timeout_ms": 5000, "allowed_hosts": [], "allowed_paths": []}}
```

### The hash is the identity and the sources are hints

A module is named by **content hash**. `sources` says where bytes matching
that hash might be found, in preference order.

A host MUST verify the hash before instantiating anything, and MUST refuse
the module when no source produces matching bytes. A source that serves
something else is a source that failed, not a module that changed.

This is the rule that makes fetching code from a stranger's URL tolerable:
the URL cannot decide what runs. **`hash` is required.** A module reference
without one is refused, rather than fetched and hoped about.

A host MAY ignore `sources` entirely and resolve the hash from a local
store. A program that runs from cache and a program that fetches run the
same bytes, which is the point of naming them by content.

### Bounds on a module are enforced by the runtime

A `module` step's bounds are the ones a WebAssembly host can actually
impose: linear memory, wall clock, and what the guest may reach.

| Bound | Meaning |
| --- | --- |
| `memory_mib` | Ceiling on linear memory. |
| `timeout_ms` | Wall clock, after which the host stops the guest. |
| `allowed_hosts` | Hosts the guest may reach. **Absent means none.** |
| `allowed_paths` | Paths the guest may read, and whether writable. **Absent means none.** |
| `fuel` | Optional instruction budget, where the runtime counts. |

**Absence is denial, never a default grant.** A host that cannot enforce a
declared bound refuses the step rather than running the guest without it,
which is the general rule in [Bounds](#bounds) applied where it is easiest
to get wrong.

A module declaring a host or a path it was not granted MUST be refused at
load, before instantiation, by inspecting what it imports. Checking after
the fact is checking after it happened.

### Module announcements — kind `30183`

Optional. A module announcement says where a hash can be found and what it
expects.

```jsonc
{
  "kind": 30183,
  "pubkey": "<publisher pubkey, hex>",
  "tags": [["d", "extract-tables"], ["hash", "sha256:9f2b…"], ["v", "1.4.0"]],
  "content": "{\"interface\":{…},\"requires\":{\"memory_mib\":64},\"sources\":[…],\"size\":184320}"
}
```

**An announcement is a locator, not an authority.** It does not say a module
is safe, and it cannot change what a program runs, because the program names
a hash. An announcement that points at different bytes fails verification
and is discarded — which is why replacement at a `d` tag is harmless here,
unlike for a program.

`requires` states what the module needs. A program's step bounds are checked
against it, and the step is refused when it grants less than the module
requires — a guest given 16 MiB when it needs 64 will fail at an arbitrary
moment instead of at admission.

Version coexistence works the way it does for programs: publish two `d`
tags. The `v` tag is for people reading a listing, and nothing resolves by
it.

### What this is a version of

Extism solved most of this and is worth reading before reimplementing any of
it. Its manifest carries Wasm sources as a file path, raw bytes, or a URL,
each with optional metadata, alongside `memory`, `timeout_ms`,
`allowed_hosts`, and `allowed_paths`. Those bounds are the same bounds, and
a capability manifest under [NIP-CAP](NIP-CAP.md) is close to the same
object.

Two deliberate differences:

- **The hash is required here and optional there.** An Extism manifest may
  name a URL with no hash, and then the URL decides what runs. For a module
  a program fetched from a registry, that is the whole risk.
- **The registry is addressable events rather than a hosted service.** A
  publisher signs an announcement and anyone can mirror the bytes, because
  identity is the hash and not the host serving it.

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

A program carries no code, so its own attack surface is the **shape** it
describes: a program that fans out to a thousand delegations, or composes to
a great depth, or names a step kind that a permissive host treats as
optional. The bounds rules above are what make those refusable, and a host
that does not enforce them is where the risk actually lives.

A `module` step changes that, and it is worth being plain about how. The
program still carries no code, but it now names code, and a host that runs
it is executing something a stranger compiled. Three things carry the weight
and none of them is the signature on the event:

- **The hash**, which means the source cannot decide what runs.
- **The import check at load**, which means a guest cannot reach a host or a
  path the step did not grant, and is refused before instantiation rather
  than caught afterwards.
- **The runtime bounds**, which mean a guest that does nothing else wrong
  still cannot run forever or allocate without limit.

Signing an announcement says a publisher wrote it. It does not say the
module is safe, and a host that treats a familiar pubkey as a reason to relax
any of the three has removed the part that was protecting it.
