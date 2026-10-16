# Native coordination

`coordination-host` implements the native coordinator, client, worker, checker,
and conditional Git integration roles of [NIP-COORD](../../nips/openagents/NIP-COORD.md).
It reuses Nostr's execution service and RUN records, the workspace artifact
store, and `workspace-host`'s actual Git reader and conditional writer.

`coordination-host --help` lists the commands. Commands require an
explicit private state directory and an owner-only key file. They never select
a repository, principal, or source from ambient configuration.

## Admission and execution

1. Initialize the store with `init`. Register the fixed `data`, `container`, or
   resource-specific `git-change` CAP definition with `operation`.
2. Mount existing workspace stores with `--sources FILE`, a JSON map from
   source names to store directories. `scope` derives each declared resource's
   actual repository/ref conflict domain and binding from its source owner.
   Caller-provided paths, alias domains, and replacement bindings are rejected.
3. Create an immutable `Policy`: recipients, independent checker keys, effect
   ceilings, calls, concurrency, claim duration, depth, and expiry. Every
   scope has its own zero-dollar root reservation. Independent monetary parent
   reservations are unsupported; pass `parent: null`.
4. Admit signed owner grants and task frames with `grant` and `frame`. A grant
   selects roles, resource IDs, operation IDs, expiry, and revocation. The owner
   remains able to inspect expired or revoked history. Frames explicitly share
   their data within the policy's admitted audience; grant artifacts and
   administrative policy bytes do not become readable through their hashes.
5. Submit an exact task proposal, then claim it with a current scope revision.
   Parent and dependency tasks must already exist. Child effects and bounds
   narrow the parent, which must explicitly permit delegation. A `container`
   reserves allowance for children without dispatching code or holding sources.
6. `prepare` produces an actual encrypted CJ execute for the named worker.
   `run` uses that worker's separate key and state directory. Its intake checks
   current rights, frame, epoch/token, expiry, input identity, and real source
   versions before exposing bytes. The native worker copies admitted JSON as
   data; a proposed Git replacement remains data at this stage.
7. `check`, using a separately admitted checker key, compares exact admitted
   input/output bytes and records a signed report. `finding` retains the
   original worker declaration and records stale inputs as history.
8. `integrate` requires current integration rights, a current exact write claim,
   the independent passing check, and the original source snapshot. The source
   owner performs its existing Git compare-and-swap, checks external writers
   again under its ref lock, and retains the source receipt. The worker never
   receives that source's write key. A required source approval still goes
   through the existing `ApprovalVerifier`; the command-line adapter supplies
   none and therefore cannot bypass such a requirement.

Every emitted capability passes the shared CAP parser. Native operation IDs
are stable slugs, and interface versions stay explicit. Resource selection is
bound by the registered definition, its exact effects, and its input schema.
The controller has separate input and result schemas, a pinned lock, context,
and requirements; its output allowance must cover at least 64 KiB plus any
returned claim resources. Retries and recovery keep that same byte ceiling. `inspect` provides its
transport bundle for independent owner admission. `invoke` produces a signed
CJ request; `serve` consumes it and returns the original encrypted response
with its exact artifact bytes. Domain conflicts are typed coordination results
inside a completed CJ answer. Private `3188` envelopes also support local
participant requests and owner administration. Neither route treats a body
field claiming an actor as authentication.

A claim's expiry prevents new intake; it never proves that earlier work stopped.
Children draw from the same durable parent ledger. Calls spent on worker
intake, checker disclosure, integration, and background observation remain
spent. Renewal and restart cannot reset them. Cancellation retains unknown
holds, and explicit release requires known outcomes and released descendants.

The controller retains one CJ identity across all scopes. After interruption,
`recover` takes the native store's exclusive mutation lock and returns the
retained domain result, or records the proven absence of a domain admission.
It does not repeat the operation. Source integration recovery likewise asks
only about its exact previously recorded source command; it never initiates
another write. Missing or unresolved worker outcomes continue to hold their
allowance. There is no controller takeover API.

## Background observations

`background` admits a finite, owner-signed plan; `poll` observes it. The native
inputs artifact is `openagents.coordination-native-background-inputs.v1`, with
`v`, empty `requires`, an exact `template` task proposal, `parent_claim`, a
bounded `sources` array, existing `watch` task ID, and `max_occurrences`.
The parent is an already admitted container claim. The template selects the
native data operation, a fixed worker context, and read-only effects covering
every selected source. The disclosure artifact explicitly repeats the scope's
recipient set.

Each observation consumes parent allowance before reading the actual sources.
Unchanged source versions/content coalesce; one effective snapshot creates one
ordinary child proposal and claim. A live or unknown child prevents another
occurrence. Foreground claims have priority. Operation completion and task
closure observe existing native state; findings never recursively trigger a
plan. Original CJ outputs are retained in the plan's evidence history.

This profile supports task/operation lifetimes, `destination: "evidence"`, and
`stale_policy: "retain_as_stale"`. Other destinations, automatic stale-work
cancellation, session lifetimes, arbitrary processes, network effects, paid
workers, general POL execution, and remote controller migration require their
own admitted adapters and refuse here. It installs no background daemon.

## Verification

`cargo test -p coordination-host` exercises actual signed producer, coordinator,
worker, checker, and source-owner paths with isolated state. Cases include
concurrent/aliasing claims, exact retries versus independent trial inputs,
revoked grants, replaced tokens, stale frames and sources, restart, unknown
holds, parent/child accounting, independent conditional integration, and
bounded background coalescing. No production account or real funds are used.


`prepare-environment` prepares a distinct ENV-bound execute for the same
admitted claim. Once prepared, its profile and bytes cannot change. The generic
worker refuses this binding. `env-host run` requires a current admitted ENV
attachment and checks the actual claim again while holding the attachment fence
through intake. It supports the native data worker; combined AUTO/ENV targets
refuse. See [the environment runtime](../env-host/README.md).
