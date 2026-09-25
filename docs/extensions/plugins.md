# Wasm plugins

Status: partly implemented. The host core and the program `module` step
landed in #9519 on 2026-09-21, and step bounds, workspace snapshot grants,
and cancellation reached the `module` step on 2026-09-25. Three evidence
guests and the program that runs them landed in #9630 on 2026-09-25, off by
default and not yet measured. The rest of this
document is the target specification for typed operations, shared
evidence, and [program execution](programs.md);
[What is built](#what-is-built) says which parts exist today.

In this document, *plugin* means an OpenAgents WebAssembly guest. It doesn't
mean a Claude Code or Codex client package under `plugins/`; the
[glossary](../glossary.md#plugins-and-skills) separates the meanings.

## What is built

Three crates implement the host core:

- [`crates/plugin`](../../crates/plugin/) is the host. `plugin::invoke`
  compiles the guest with Wasmtime and runs it in a fresh store and instance
  for every call, so no memory or handle survives between calls. It links no
  WASI and no other ambient interface.
- [`crates/plugin-pdk`](../../crates/plugin-pdk/) holds the
  `openagents.plugin-packet.v1` request and response types that the host
  and a guest share.
- [`crates/plugin-outline`](../../crates/plugin-outline/) is a diagnostic
  guest. Its `echo` operation returns its input, and its `outline`
  operation lists a granted snapshot. The compiled guests are the fixtures
  in `crates/plugin/fixtures/`.

With the `guest` feature, `plugin_pdk::guest` is the guest side of the ABI:
`export_guest!` exports one handler as `oa_alloc`, `oa_free`, and
`oa_handle`; `list`, `size`, and `read` wrap the `oa_host.call` import; and
`MemoryHost` answers the same calls from files in memory, so a guest's
logic runs in native tests. Three guests are built on it, as
[Evidence guests](#evidence-guests) describes.

The host implements the two access modes as `plugin::Profile`:

- `Pure` links no host imports. A guest that declares any import is
  refused with `HostError::Denied`.
- `SnapshotRead` links one import, `oa_host.call`, which lists and reads
  the handles issued for that invocation. Listing a directory mints a
  handle for each child, scoped to the invocation, so the guest can read
  what it listed. A handle from another invocation is `HostError::Stale`,
  and path traversal, symlink escape, and a partial capture are refused.
  An entry name is a logical label; it may have `/`-separated segments,
  but no segment is empty, `.`, or `..`.

`plugin::Limits` bounds each call. The defaults are the following:

| Limit | Default | What it bounds |
| --- | --- | --- |
| `fuel` | 1,000,000 | Guest instructions, including the start function. |
| `memory_bytes` | 8 MiB | Guest linear memory. |
| `output_bytes` | 64 KiB | The response body. |
| `read_bytes` | 64 KiB | Snapshot bytes that one call may read. |
| `module_bytes` | 2 MiB | Guest module size accepted for compilation. |

Setting the call's cancel flag stops a running guest, not only one that is
about to start or is in a host call: a watcher thread bumps the Wasmtime
epoch, and the guest traps at its next loop header or function entry with
`HostError::Cancelled`.

A call that doesn't return a value fails with a typed `HostError`:
`Malformed`, `Denied`, `Limit`, `Stale`, `Failed`, `Cancelled`, or
`Refused`. The host checks pointer and length ranges, refuses output that
overlaps the input, and reports a spent `fuel` allowance as a limit rather
than a trap. A returned value always carries `verification: not_run`,
because a guest doesn't verify its own output.

`plugin::build_receipt` binds a guest build to the PDK source it was
compiled against: the SHA-256 digests of the PDK source and the guest bytes,
and the profile the guest was built for.

In `crates/coder`, the program runtime runs a `module` step through
`plugin::invoke` (`run_module` in `crates/coder/src/runtime.rs`), on the same
path the terminal and `coder -p` share. Admission refuses a `module` step
that carries no guest bytes (`bytes_base64`) or names a profile other than
`pure` or `snapshot-read`.

The runtime holds each step to the host's ceilings, which are the defaults
in the table above except `fuel`, which is 50,000,000. An operator can set
other ceilings with `Runtime::with_module_ceiling`. A step's `fuel`,
`memory_bytes`, `output_bytes`, `read_bytes`, and `module_bytes` bounds
reach the host and can only narrow those ceilings: admission refuses a
bound wider than its ceiling as `bound_unenforceable`, and a guest that
crosses a narrowed bound fails the step with the host's typed limit, such
as `limit_exceeded` with `output bytes`. A child program's `module` step is
held to the same ceilings, and a `program` step can't widen them for it.

A `snapshot-read` step reads only what its `read` field names: a list of
workspace-relative paths, where `.` names the whole workspace. A step with
no `read` field is granted nothing and sees an empty listing. The safer
default is to grant nothing: a program that forgets to scope its guest
then shows an empty listing, rather than silently handing the guest every
file in the checkout. Before the guest starts, the runtime reads each named
file, and each file under a named directory, from the run's workspace into
the snapshot. The guest gets one handle, `workspace`, whose listing names
each file as `workspace/<relative path>`. Each file keeps at most the
step's `read_bytes` and says when it kept fewer bytes than it has. A grant
holds at most 1,024 entries and 16 MiB. A path that isn't plain and
relative, such as one with `..`, is refused at admission as
`scope_invalid`. A path that resolves outside the workspace, or a symlink
whose target is outside it, refuses the step as `scope_escapes` before the
guest starts. A symlink whose target stays inside is listed as a symlink
and never followed. A `pure` step gets no snapshot and no handles, and a
`pure` step that names a `read` scope is refused at admission.

The following step, in a program's host binding or a host program document,
lists the files under `docs/` and `README.md`, and reads no more than 4 KiB
of them:

```json
{
  "name": "outline",
  "kind": "module",
  "module": {
    "profile": "snapshot-read",
    "operation": "outline",
    "read": ["docs", "README.md"],
    "bytes_base64": "<guest bytes>"
  },
  "bounds": {"read_bytes": 4096}
}
```

The `read` field lives in the host's step binding beside `bytes_base64`,
not in the portable NIP-PRG definition, because which paths a guest may
read is the machine's business, like the paths a source reads. The
`crates/nostr` definition parser doesn't see it.

The guest runs on a blocking thread under a cancel flag. The run's
deadline sets the flag and waits for the guest to stop, and the step marks
`cancelled` with the run. A run the caller drops sets the flag too. A guest
refusal, a crossed limit, a denied import, and a cancellation map to the
step's `refused`, `limit_exceeded`, `denied`, and `cancelled` refusals.

The following aren't built yet:

- The manifest, operation schemas, and compatibility checks in
  [Manifest and compatibility](#manifest-and-compatibility). The host takes
  guest bytes and an operation name; it doesn't read a manifest.
- Snapshots of anything other than workspace files, such as a document,
  a dataset partition, or a service capture.
- A wall-time bound of the host's own, a host-call count bound, and a
  compilation cache. Each call compiles the module again. The only wall
  bound on a guest is the run's deadline.
- The [host roles](#host-roles): evidence preparation, output processing,
  and hook registration.
- The full [build provenance](#authoring-and-build-provenance) and the
  [invocation receipt](#invocation-receipt). The build receipt holds only the
  three fields above, and the run records a module step's output, not a
  digested invocation receipt.
- The authoring surface: no command initializes, packages, installs, or
  lists a plugin. `scripts/build-plugin-guests.sh` builds and pins the
  three evidence guests, and nothing else.
- Resolving a module by digest in the product, and a remote plugin
  catalog. A program carries its guest's bytes inline;
  `programs/evidence-guests.json` is the one program in `programs/` that
  uses `module` steps. The
  [interoperability suite](../coder/verification/2026-09-22-relay-interoperability.md)
  locates a guest over a relay and runs it, but only in a test.

## Evidence guests

The pre-reset Coder evidence plugins, removed in `dabc08102f`, went unused
when they were offered to the model as tools. Three of them are rebuilt as
`snapshot-read` guests that code calls as program steps. They reimplement
the old plugins' purpose on the `openagents.plugin-packet.v1` ABI rather
than port their code, which targeted a different ABI with mounts.

| Guest | Crate | Operation | Input | Output |
| --- | --- | --- | --- | --- |
| Repository map | `crates/plugin-repo-map` | `map` | `max_files` | Files and bytes, languages by extension, top-level entries with their file counts, the directories one level below, the largest files, build manifests, and test files. It reads sizes, not contents. |
| Code search | `crates/plugin-code-search` | `search` | `patterns` (1 to 16), `case_sensitive`, `whole_word`, and result bounds | Matching lines grouped by file, files that match more patterns first, with per-pattern counts. A pattern is literal text where `*` matches any run within a line. Binary, lock, and minified files are skipped. |
| Test report | `crates/plugin-test-report` | `parse` | `paths`, or none to pick files by name, and `max_failures` | Each report's format and counts, and each failing test with its file, line, and message. The format comes from the content: JUnit XML, `cargo test` output, or pytest output. |

Every output counts what its bounds left out, such as `truncated`,
`files_unread`, or `complete: false`, rather than dropping it silently. Each
crate has fixtures under `fixtures/tree` and native tests that run the
guest's logic against `MemoryHost`.

### Build and receipts

Run the following command to build the three guests:

```sh
./scripts/build-plugin-guests.sh
```

It builds each crate for `wasm32-unknown-unknown` with the pinned 1.97.1
toolchain and the workspace's `guest` profile (`opt-level = "z"`, LTO, and
`panic = "abort"`), with the checkout and Cargo home paths remapped so the
bytes don't depend on the machine. Two builds in separate target
directories produce the same digests. The script then does the following:

1. Copies each module to `crates/plugin/fixtures/<guest>.wasm`, following
   the `outline.wasm` convention.
2. Writes `crates/plugin/fixtures/<guest>.receipt.json`: the
   `plugin::build_receipt` fields (the PDK source digest, over
   `plugin-pdk/src/lib.rs` and `guest.rs`, the module digest, and the
   profile), plus the guest source digest, over its `Cargo.toml` and
   `src/lib.rs`, the size, the toolchain, and the target.
3. Inlines each module into `programs/evidence-guests.json` as its step's
   `bytes_base64`, and pins the module's digest and size in the step's
   target.

`crates/plugin/tests/guests.rs` runs each checked-in module through
`plugin::invoke` over its crate's fixtures, and fails when a receipt no
longer matches the module, the PDK source, or the guest source. Edit a
guest or the PDK, then run the script.

### How Coder One runs them

`crates/coder-one/src/guests.rs` is a host for the program, because Coder
One doesn't depend on `crates/coder`. It holds a step to the same rule as
the terminal's runtime: a step's bounds narrow the host's ceilings and
never widen them. It also refuses a step whose inline bytes don't match the
digest its target pins. The program's bounds are wider than the terminal's
default ceilings, so the terminal refuses the program at admission and never
offers it to the program selection question.

The switch is `policy.evidence.guests` in a Coder One policy manifest. It is
absent by default, and absent leaves the manifest's digest unchanged. It
names the steps to run and the seconds they share:

```json
"evidence": {
  "probes": "v2",
  "survey_files": 40,
  "guests": {"steps": ["repo_map", "code_search", "test_report"], "seconds": 10}
}
```

`{}` means all three steps and 10 seconds. The switch needs
`evidence.probes`, because the guests run in the probe stage, after the
probe battery. Code decides what each step does:

- `repo_map` always runs.
- `code_search` runs only when the issue yields search terms, and searches
  for up to eight of them.
- `test_report` runs only when a granted file's content shows a test
  report, and parses only those files.

The grant holds the workspace's files, without `.git`, `target`,
`node_modules`, virtual environments, caches, or symlinks, at most 20,000
files, 64 KiB of each, and 64 MiB in all. Code orders it, and a guest reads
in that order: shallow paths before deep ones, such as a checked-in trace
archive, then paths that name a search term, then source files. What the
grant left out or cut is stated in each output. Each output becomes a
probe output labeled `guest repo-map`, `guest code-search: <terms>`, or
`guest test-report`, and faces the probe keep question with the battery's
outputs, so what reaches the briefing is still Jev's choice. The run is
recorded as the `evidence.guests` invocation.

### Measurement plan

No guest is kept because it exists. Each is kept only if it helps on the
[issue-flow evaluation set](../coder/guides/coder-one-issue-eval.md)
(#9625) or on the [mini-tasks](../coder/guides/coder-one-minitasks.md),
measured against a matched baseline. No live run is part of #9630.

1. **Arms.** The baseline is `issue-flow.json` unchanged. The treatment arms
   add `policy.evidence.guests` with all three steps, then with each step
   alone. Everything else in the manifest is the same, so each arm's digest
   differs from the baseline's only by the switch.
2. **Matching.** Each arm runs the same entries at the same base commits,
   with the same model, deadlines, seal, and number of repeats, through
   `coder-one issue-eval run ID --policy PATH`. Work on the development
   part; run the held-out part only to confirm a guest that is already
   chosen.
3. **Targeted first.** Before a whole-set run, run one or two entries where
   a guest should matter, such as a Rust behavior entry for the test-report
   parser. Run the whole set only if the targeted runs show a large change.
4. **Measures.** Per entry and arm: graded pass or fail, Luna and Jev cost,
   wall time, the characters each guest put in the briefing, how often the
   probe keep question kept each guest's output, and each guest's time,
   skips, and refusals from the `evidence.guests` record.
5. **Rule.** A guest stays in the default steps only if its arm passes at
   least as many entries as the baseline and improves pass rate, cost, or
   time beyond the spread between repeats. A guest whose output the keep
   question almost never keeps, or that changes nothing, is dropped from the
   default and then from the program.

## Guest boundary

A plugin is a content-addressed WebAssembly guest with a manifest, typed input
and output, and bounded host imports. It computes or reads explicitly granted
snapshots. Rust owns invocation, authority, storage, decisions, and effects.
A module's bytes, manifest, schemas, and operation descriptors have separate
digests bound by the package release. Matching the module digest alone does
not establish its interface or permissions.

The [NIP-PRG guest profile](../../nips/openagents/NIP-PRG.md#wasm-module-profile)
specifies two access modes:

| Mode | Available operations | Admission |
| --- | --- | --- |
| Pure computation | Transform the supplied packet; no ambient filesystem, process, network, clock, randomness, or credentials. | Installed and enabled component, supported ABI, validated inputs, and resource allowance. |
| Snapshot reading | Pure computation plus bounded reads from named, immutable evidence or repository snapshots. | All pure-mode checks plus explicit host grants for the named snapshots and readable scope. |

Neither mode permits writes to the workspace, process spawning, network
requests, model calls, package installation, or changes to policy. Refuse a
manifest requiring these effects. A plugin may return a proposed patch or
structured test invocation as data; a separately authorized native operation
validates and applies it. The plugin cannot turn its return value into a
command by choosing a special string.

Network-capable adapters and agent executors remain separate host components.
A future guest profile requires its own version, threat model, and acceptance
evidence. The revised NIP-PRG profile refuses these broader effects;
unsupported required bounds cause admission refusal.

## Manifest and compatibility

Each plugin manifest must declare:

- Publisher-qualified identity, version label, exact module digest and byte
  size, license, source provenance, and compatible host/ABI versions.
- Named operation exports and their versioned input/output schemas, maximum
  packet sizes, supported formats, and partial-result semantics.
- Required access mode, supported imports, read scopes, and resource minima
  and requested maxima. Optional access must be explicitly distinguished;
  absence changes a typed result, never silently broadens access.
- Host roles, event/input schemas, eligibility conditions, and output
  representation types. Exporting an operation grants no automatic role.
- Determinism and idempotency claims, including any dependencies on parser
  versions, snapshot metadata, locale, or other supplied inputs.
- Fixtures and conformance evidence references. These are author claims until
  the host or an independent evaluator verifies the identified artifact.

Reject unsupported manifest versions, unknown required imports, ambiguous
exports, schema/digest mismatches, duplicate identities, and limits the host
cannot enforce. Version labels are for people; digests select executable
bytes. An ABI-compatible update is still a different artifact requiring an
explicit update and fresh admission.

The engine is Wasmtime, which preserves the reference design's practical
Rust embedding. `crates/plugin` pins Wasmtime 48. Bind the engine version and
configuration in the host receipt; the current host doesn't record them yet. Engine choice does not make guest output trustworthy or prove
determinism. No ambient WASI access is enabled by default.

## Typed packet ABI

Implement the [NIP-PRG packet ABI](../../nips/openagents/NIP-PRG.md#packet-abi),
with a Rust PDK hiding allocation and serialization. `crates/plugin-pdk`
provides the packet types, and `crates/plugin/tests/abi.rs` covers the
memory, import, fuel, stale-handle, and cancellation cases. Conformance
vectors for the full contract cover:

| Boundary | Required contract |
| --- | --- |
| Invocation | ABI version, host-generated invocation ID, exact operation ID, input schema version, typed value, and scoped evidence handles. |
| Return | A discriminated success, unsupported-input, or refusal value; exact output schema; bounded value and source references. |
| Memory | Explicit allocation/deallocation ownership; checked pointer/length arithmetic; valid guest-memory ranges; no reused stale references. |
| Encoding | One documented serialization, duplicate-key and unknown-field policy, numeric/string limits, and rejection of trailing or malformed input. |
| Imports | Versioned typed calls with per-call and aggregate bounds; scoped opaque handles rather than ambient paths. |
| Failure | Distinct host validation failure, guest refusal, trap, deadline, cancellation, memory exhaustion, and output-limit failure. |

NIP-PRG fixes binary exports and serialization. Generated PDK types and
fixtures must agree with that contract; packages must not advertise
compatibility before their implementation passes it.
The PDK's public programming model is a typed input-to-result function. It
must not encourage plugins to write tool-call envelopes or imitate a model.

Validate input before entering the guest. Validate output before committing
evidence or passing it to a consumer. The host assigns source scope,
invocation identity, capture completeness, and authority metadata; a guest
cannot assert these fields to certify itself. Treat strings inside a valid
packet as untrusted data when building later model inputs.

## Reads and resource accounting

Snapshot imports expose only named handles and bounded operations such as
metadata lookup, directory listing, and byte-range reads. Reject path
traversal, symlink escape, out-of-range reads, and handles from other runs.
The host must resolve a grant against stable object identities. If it cannot
provide a stable snapshot, record versions before and after and reject stale
results rather than claim an immutable read.

Bound compilation, wall time, guest memory, instruction work, packet sizes,
host-call count, aggregate bytes read, output bytes, and retained evidence.
Check declared lengths before allocation. Host imports consume the same
deadline and budget as guest work. An instruction meter alone does not bound
a blocked import. The host must be able to interrupt a guest and cancel its
imports; admission refuses unenforceable requirements.

Use a fresh invocation state. A compilation cache may reuse verified module
code by module digest, engine version, and configuration; it must not share
mutable memory or evidence handles across tasks. Snapshot access contributes
to the cache identity of a result. A manifest's deterministic claim is not
sufficient to cache a result across incompatible inputs or host policies.

Ceilings are explicit host configuration constrained by parent budgets. The
current defaults are in [What is built](#what-is-built); they aren't yet
measured against real guests. Do not assume universal
numeric limits merely because they were once convenient. Records must show
the effective limits, consumption, and any unknown values.

## Host roles

The target host supports three roles inherited from Coder, with narrower
activation than a general event bus. Only the explicit operation, through a
program `module` step, is built:

| Role | Trigger and result | Control |
| --- | --- | --- |
| Evidence preparation | An admitted context request asks for a repository map, outline, facts, or another derived representation. | Host selects bounded operations and stores results with source references; no unconditional per-turn prelude. |
| Output processing | A supported native operation captures output and requests a typed derivative. | Host checks command/output format eligibility and binds the derivative to that capture. |
| Explicit operation | A program `module` step or approved operation call supplies typed inputs. | Runtime admits the exact component, arguments, authority, and limits. |

Loading a package does not register arbitrary executable hooks. Role
registration declares supported event and schema IDs; activation requires
host policy and a bounded task or session scope. A hook cannot recursively
trigger itself, invoke other hooks, or extend its own lifetime. Programs
express explicit multi-step composition instead.

For automatic output processing, one host rule owns the selected
representation for a capture. Multiple derived views may be computed
explicitly when useful, but completion order cannot determine which view
reaches the model. Record the winning rule and alternatives. Typed program
dataflow can chain transformations; installation order cannot create a chain.

## Evidence transformations and fallback

Retain the original bounded capture under the host's evidence policy and
store derivatives separately. Each derivative names the original evidence
version, plugin and schema digests, transformation parameters, capture limits,
and any omissions. A ten-line summary of truncated output is still derived
from incomplete evidence. It cannot restore lines that were never captured.

The context builder may use an outline, diagnostic summary, matching spans,
or raw text and retain a path to expand the original. Format validation is
mechanical; whether a representation answers the task may require a separate
admitted decision function. A guest does not declare its output sufficient
for every consumer. Never replace mandatory error, verification, instruction,
or acceptance evidence solely because a shorter representation exists.

On unsupported input, malformed output, trap, timeout, or optional-role
failure, preserve the original and follow the host's bounded fallback policy.
Fallback may select a safe bounded raw view or report that more evidence is
needed. It must not inject an unbounded original into a model context. An
explicit required module step follows its declared program failure policy;
it does not silently become a successful no-op.

## Plugins in optimization studies

A plugin may be an implementation dependency or prepare evidence for a study.
An optimizer can propose new guest bytes only within an admitted source/build
surface. Build, schema, sandbox, and effect validation apply before measurement.
The study identifies the exact loaded guest and host imports.

Pure and snapshot-read guests do not acquire model, network, or process access
by being generated by DSPy, GEPA, or another authoring tool. Model inference
runs through a separately admitted host operation. Optimizer and evaluator
execution also require their own capabilities and reservations.

Prompts and examples are not Wasm modules. Export only supported component
types and pinned assets, and refuse executable foreign serialization. The
[optimization architecture](../optimization/architecture.md) defines those
boundaries and the evidence needed before adopting a generated component.

## Authoring and build provenance

Provide a Rust PDK, starter manifests, fixtures, and a shared authoring service
used by the CLI, terminal, and any model-facing tool. Expose closed typed
operations for initialize, build, inspect, test, package, install, list, and
uninstall. These are proposed surfaces, not commands available today. Creating
a project writes files under a selected directory; building runs native
tooling under ordinary host execution authority. A Wasm sandbox does not
sandbox Cargo build scripts.

The current `plugin::BuildReceipt` binds the PDK source digest, the guest
digest, and the profile. The target build receipt must bind the source snapshot, all relevant local dependencies,
lockfile, hidden configuration, build scripts, toolchain, build inputs,
artifact and manifest digests, and completed validation. Record pending,
failed, and completed builds distinctly. A fresh isolated build directory and
locked dependencies reduce stale-artifact mistakes; they do not establish
hermeticity or supply-chain attestation.

Check source identity before and after building. Refuse packaging or
publication from a stale, failed, or unmatched artifact. Fixture success on
one binary cannot certify another. Local installation may use an independently
verified prebuilt release without a local build receipt; its provenance must
say so. No model-facing authoring call gains broader filesystem or publication
authority than the equivalent human-facing operation.

## Invocation receipt

Record component/manifest/schema digests, host and ABI versions, invocation
and originating program/step/call IDs, input/output digests, source evidence,
grants and policy, effective limits, timing, consumption, and typed outcome.
Record fallback and whether a derivative was selected, merely produced, or
rejected. Sensitive payloads stay in scoped evidence storage; a receipt need
not duplicate them into a globally readable log.

Receipts establish attribution and local consistency. They are not remote
attestation, a correctness proof, or evidence that fewer output bytes improved
the complete task. See [evaluation](delivery.md#evaluation-and-default-admission).
