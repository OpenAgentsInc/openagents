# Plugins

A plugin is anything you add to OpenAgents. You write a plugin, test it
with Coder and without it, and publish it so other people can check your
result and use it.

This page is the starting point for people who make plugins. The
engineering specifications behind it stay in
[`docs/extensions/`](../extensions/README.md), and the
[glossary](../glossary.md#one-vocabulary-what-you-can-add) maps the word
*plugin* onto the precise terms those specifications use.

The [Brainstorm integration proposal](brainstorm-v1-integration.md) describes
an opt-in Coder V1 plugin for public Nostr account discovery and reputation,
with delivery scope for October 7, 2026.

The [Brainstorm guidance companion](../../plugins/brainstorm/README.md) is a
self-contained skills-only package for the separately enabled native Coder
binding. Its exact host/source requirement is descriptive. Installation leaves
guidance off, installs no native code, and performs no lookup or publication.
Its optional private pilot checker preserves exact-key coverage and distinguishes
operator-recorded funnel stages from qualified paid conversion.

The [meeting follow-up template](../../plugins/meeting-followup/README.md)
packages the existing action-items guest for an operations department. Its
local runner binds exact input/release and current recipient/permission
snapshots; independent protected checks stay outside the guest's read scope.
Synthetic comparisons establish bounded extraction, not customer ROI.

The [meeting action-items release](../../plugins/meeting-action-items/README.md)
uses the same guest on notes supplied in the request, within the current paid
route's single-step, empty-snapshot contract. Its signed publication and payment
fixture pins the exact release and full author fee. Real publication, independent
buyer acceptance, funded settlement, and payout still need owner qualification.

## What a plugin contains

A plugin can contain any of these parts:

| Part | What it is | Where it lives in a plugin directory |
| --- | --- | --- |
| Skills | Instructions the agent reads before a task, as Markdown files (`skills/<name>.md`). A skill runs no code and grants nothing. | `skills/` |
| Workflows | Typed, step-by-step programs that a host runs: look things up, check, decide, delegate, or run Wasm. Workflows name what they use by digest. The protocol calls a workflow a *program* ([NIP-PRG](../../nips/openagents/NIP-PRG.md)). | `programs/`, pinned by `program` in `package.json` |
| Knowledge | Cited reference entries: a method, an edge case, a common slip, or how a command is used ([NIP-KB](../../nips/openagents/NIP-KB.md)). | Published as NIP-KB entries with `openagents kb publish`, beside the plugin's release |
| Wasm | Sandboxed WebAssembly code that performs one bounded operation, such as mapping a repository's files. It is the only executable code a plugin can carry. It runs with no network, under fuel and memory limits, and reads only what the host hands it. A workflow runs it; a model never calls it by name. | A guest crate built against [`crates/plugin-pdk`](../../crates/plugin-pdk/) |
| Tests | The test set that shows whether the plugin helps: tasks Coder runs with the plugin and without it, and the checks on each run. | `evals/` |

A plugin can also bring **background rules**: a rule the host runs on its
own, on a timer or when a task ends, only while the plugin is turned on
on that computer. See [Plugins that run in the background](#plugins-that-run-in-the-background).

Not every plugin has every part. A plugin made in chat is a single skill.

Coder and the coding agents it works with (Codex, Claude Code, Grok Build,
OpenCode, and Devin) are not plugins. They are the agents plugins plug
into.

We don't call any part of a plugin a *tool*. In AI products a tool is
something a model chooses to call by name. A plugin's Wasm is chosen and run
by workflows and typed decisions, never by a name a model writes.

## Plugins you can use now

| Plugin | What it does | Source |
| --- | --- | --- |
| Claude Code | Coder hands a task to Claude Code on your computer and shows its progress as it works. | [`crates/coder-new/src/acp_discovery.rs`](../../crates/coder-new/src/acp_discovery.rs) |
| Codex | Coder hands a task to Codex on your computer and shows its progress as it works. | [`crates/coder-new/src/acp_discovery.rs`](../../crates/coder-new/src/acp_discovery.rs) |
| Cursor | Coder hands a task to Cursor's agent on your computer and shows its progress as it works. | [`crates/coder-new/src/acp_discovery.rs`](../../crates/coder-new/src/acp_discovery.rs) |
| Grok Build | Coder hands a task to Grok Build on your computer and shows its progress as it works. | [`crates/coder-new/src/acp_discovery.rs`](../../crates/coder-new/src/acp_discovery.rs) |
| OpenRouter | Use OpenRouter models in Coder with your own API key. | [`crates/coder-new/src/plugin_definition.rs`](../../crates/coder-new/src/plugin_definition.rs) |

These are the plugins built into Coder in the `openagents` terminal
(`/plugins` there turns each on or off). The coding agents are found on
your computer when they are installed, and Coder delegates to them over
ACP. This table, the chat's answers about which plugins there are, and the
website's plugin cards follow
[`crates/coder/src/builtin_plugins.rs`](../../crates/coder/src/builtin_plugins.rs);
`cargo test -p coder --test plugin_catalog` fails until they agree. After
changing that list, rewrite the generated plugin list and the route map:
`PLUGIN_LIST_WRITE=1 cargo test -p coder --test plugin_catalog` and
`ROUTE_MAP_WRITE=1 cargo test -p coder --test route_map_sources`.

The packages under `crates/plugin-*` that the hosted eval runner lists
([`deploy/eval-runner/catalog`](../../deploy/eval-runner/catalog)) are test
fixtures for the runner. They are not shown to people.

## Reuse an exact release with a colleague

On a Unix host, `openagents plugin team` joins the current native account and
workspace to the existing signed release, evaluation, installation, and Wasm
owners. Select an absolute private `--registry DIR`, an explicit mode-0600
`--credential FILE`, and `--workspace ID`. It uses no ambient gateway key,
provider login, relay, wallet, or creator workspace.

Use `team inspect` to discover scoped evidence. `team prepare` returns an exact
request and its approval digest without granting authority. Inspection separates
source qualification from current action rights and reports insufficient rights,
withdrawal, unavailable evidence, and unqualified releases. A current owner or
admin uses `team grant --request FILE --approve DIGEST` to accept one member,
release, operation, input byte digest, purpose, local output recipient, and
expiry of at most 30 days. The reviewer must separately hold the input's rights.
Current credential scopes must permit `team-capabilities.read` and the selected
`team-capabilities.review`, `.enable`, or `.use` action; read access alone
cannot enable or run a release.

The admitted colleague uses `team install`, which leaves the plugin off, then
`team enable`, then `team use --input FILE --approve-input DIGEST`. These
commands need an explicit `--grant ID` and unique `--operation-id ID`. Every
new operation rechecks the recipient membership epoch, action rights, expiry,
source requirements, signed publisher listing and revocation checkpoint, exact
release and artifact bytes, and scoped evaluation references. A changed release,
recipient, source, operation, or input needs a new review. `team revoke` ends new
use. Admitted receipts retain the original revision; completed retries return
receipt metadata without rerunning or retaining private input or output in the
account book. An interrupted operation stays unknown and needs inspection and a
new explicitly reviewed operation.

The supported native lane is a signed zero-fee, single-program Wasm package with
only `package.json` and its selected program. Its curation review declares
`data_requirements: ["explicit-request-text"]` and `recipients: ["local-wasm"]`.
It runs the existing packet host with the approved UTF-8 request and an empty
snapshot, under the program's fuel, memory, and output bounds. Background rules,
cache classes, capabilities, creator examples, protected labels, network access,
and paid releases require their separate owners. Signed evaluation claims remain
scoped evidence and grant no installation, data, execution, or payment right.
The source mirror can withhold newer records; fresh signed checkpoints and
retained head knowledge cannot prove global completeness. Real team data rights,
publication, and colleague delivery remain in `NEEDS_OWNER.md`.

## Read your purchases in the browser

`openagents-web --customer DIRECTORY` serves a read-only purchase browser at
`http://127.0.0.1:4300/app/purchases` on the machine that holds the customer
store. Each purchase shows the phase, release, payer node, price and fee
bound, quote digest, approval digest, charge, settlement, and delivery that
the installed `openagents plugin purchase` client recorded, and names the
exact command that resumes it there. The browser never opens the store for
writing, takes its lock, or reaches a wallet: quote, approve, invoke, cancel,
and recover stay on the installed client, and reloading a page cannot pay or
dispatch twice. Without `--customer`, the page says purchases are
unavailable.

## Buy a step that reads files you supply

A paid release may declare, in its program's `module.read`, up to eight
logical file names its `snapshot-read` guest reads. To buy such a step,
supply exactly those files with the request: `openagents plugin purchase
quote ... --input FILE --file meeting.md=PATH`, then `approve` and `invoke`
as for any purchase. The client sends the files as UTF-8 text in the request
body (`snapshot.v = openagents.plugin.supplied-snapshot.v1`), the quote and
approval digests cover them, and the invocation receipt's `snapshot` field is
the digest of that supplied snapshot. The guest sees nothing but those bytes:
no workspace, capability, network, process, or wallet. A missing, extra,
renamed, or oversized file, a release that declares no names, or a release
with any broader grant is refused before payment. Releases that read nothing
keep their empty snapshot and their existing terms.
`plugins/meeting-followup` is the example release; its fixture is
`plugins/meeting-followup/examples/meeting.md`.

## Inspect a curated publisher or service

On a Unix host, `openagents plugin discover --catalog FILE --mirror DIR --json`
inspects an explicitly selected source set. It contacts no relay, reputation
service, provider, or wallet. Add `--query TEXT` to rank signed names and exact
identities, or `--select KEY:PACKAGE/OPERATION` to print one admitted card.
Selection prints evidence and grants no installation, disclosure, execution,
or spending authority.

The catalog is a local operator admission with `schema` set to
`openagents.discovery.curated.v1`, an attributed `curator`, positive
`max_age_seconds`, `skew_seconds` from 0 through 300, and at most 64 `items`.
Each item names `id`, `kind`, exact signed `event`, `operation`, `digest`,
`evaluations`, optional `review`, and optional advisory `reputation`:

- An `extension` uses the publisher-qualified package ID, exact NIP-EXT release
  ID and manifest digest, and a component slug. The reader checks the current
  signed listing, manifest, complete bounded file closure, and fresh signed
  publisher revocation checkpoint. Dependency closures are unavailable in this
  selected profile.
- A `service` uses the full qualified NIP-CAP component ID, exact service-head
  ID, canonical definition digest, and advertised door name. It checks the
  publisher, interface, transport, door, and freshness. A service publication
  does not establish a current price or live capacity.

The explicit mirror contains signed events at `events/ID.json`, current signed
heads at `heads/KIND/PUBLISHER/SLUG.json`, and exact bytes at
`artifacts/sha256/DIGEST`. A head file can retain a bounded signed-event array;
the newest head wins, including hidden or malformed newer publications.
Locator hints never cause network requests. Symlinks, nonregular objects,
altered bytes, substituted keys, and excessive source sizes refuse.

Retain the complete JSON output and pass it as `--previous FILE` on the next
inspection. Its signed evidence preserves head watermarks and monotone publisher
revocations across source changes. A withheld newer record remains a verification
limit; fresh evidence does not prove global completeness. This selected reader
does not advertise general NIP-REG conformance.

Cards separate lexical relevance, advisory reputation, signed publication,
native supported operation, availability, price, measured evaluation, and local
review. Native packet inspection reuses the paid-plugin owner's exact single-step
guest validator and runs nothing. The signed publisher fee excludes endpoint and
fulfillment charges; an absent fee remains unknown. The owning customer path
must resolve and approve a separate current total quote.

An optional `review` records `reviewer`, `reviewed_at`, `valid_until`, and the
exact item `event`, `digest`, `operation`, and `evaluations`, plus
`publisher_fee_msat`, `data_requirements`, `recipients`, and `limitations`.
A current scoped review requires those pins and the explicit signed fee to
match current evidence, with passing signed measurements for the exact subject.
The reader verifies the published suite, case scope, and subject run lock; it
does not rerun the cases. Missing, stale, conflicting, or unrelated evidence
remains unqualified. A reviewed quote candidate still requires provider and paid
lane qualification, current customer authority, and explicit purchase approval.

## Make a plugin in chat

In the OpenAgents app on your phone, ask "Help me make a plugin that ...".
We draft the plugin and its tests with you one step at a time, try it once,
and then run the full test set on our computers. A plugin made in chat is a
skill. A
plugin that needs new code goes to Coder on a computer you connect.

In a terminal on your computer (`openagents chat` or OpenAgents Terminal),
the same request makes the plugin there, in steps
([#10177](https://github.com/OpenAgentsInc/openagents/issues/10177)):

1. If your request doesn't say what the plugin should do, we ask what it
   should do and what it shouldn't.
1. Coder drafts it in the project's `plugins/<slug>/` folder, in its own
   worktree: the package record, a skill under `skills/`, a README, and its
   tests under `evals/`. It installs, publishes, and runs nothing.
1. We show you the tests. Say yes, or what to change, and Coder changes
   them.
1. We run the tests on this computer, with the plugin and without it
   (`openagents plugin test run`), and show the result.
1. We ask whether to publish it and turn it on here, and run what you
   choose: `openagents plugin publish` (see
   [Publish a plugin](#publish-a-plugin)), and
   `openagents plugin install` and `openagents plugin enable`.

A plugin that only brings skills needs no workflow: its package record names
no `program`, and `skills/` holds at least one `.md` file.

A reviewed draft can also be retained with `openagents plugin workbench freeze
--root DIR --review FILE`. The typed review contains `flow`, the existing chat `snapshot` with its
original Coder task binding and routed plugin interview, `directory`, and
author/fee/payout `declarations`. The snapshot is owner-supplied evidence;
freezing never starts or verifies an engine run.
`show` reads its retained record and reviewed draft files; `approve --request FILE` names the exact
package and tree digests and one separate comparison, publication,
installation, enabling, or independent reuse choice. The native terminal and Verse
`--capability-flow DIR` mount the same read-only workbench pane. Unknown
actions are retained without replay; a failed comparison remains evidence,
and unreported costs and unsettled fees stay labeled as such. Reuse names a
distinct owner-declared `source_task`; the retained result contains the actual
route request, thread, artifacts, and checks returned by the use engine.

## Write a plugin

A plugin is a directory with a package record, `package.json`, at its
root. The record names the plugin and pins its workflow by digest:

```json
{
  "v": 1,
  "slug": "project-map",
  "name": "Project map",
  "summary": "Shows Coder how the project is laid out before it starts.",
  "version": "0.1.0",
  "publisher": "<your 64-hex public key>",
  "program": { "name": "project-map", "digest": "<sha-256 of programs/project-map.json>" }
}
```

Add the parts your plugin needs:

1. Put guidance in `skills/<name>.md`; the test run reads every `.md`
   file directly in `skills/`.
1. Put the workflow in `programs/<name>.json`. The
   [workflow guide](../programs.md) covers step kinds, sources, and
   bounds.
1. Build Wasm as a guest crate against `crates/plugin-pdk`. The
   [Wasm host specification](../extensions/plugins.md) covers profiles,
   limits, and build receipts.
1. Write knowledge entries under `knowledge/` and check them with
   `openagents kb`.

## Plugins that run in the background

A plugin can bring rules the host runs by itself, such as keeping the disk
from filling up. Each rule is a JSON document under `background/`, in the
background rule format (`openagents.background.rule.v1`, see
[`crates/background/src/rule.rs`](../../crates/background/src/rule.rs) and
the [background processes spec](../background/2026-10-02-background-processes.md)),
pinned in `package.json`:

```json
{
  "v": 1,
  "slug": "disk-cleanup",
  "name": "Disk cleanup",
  "summary": "Keeps the disk from filling up.",
  "version": "0.1.0",
  "background": [{ "name": "disk-cleanup", "digest": "<digest of background/disk-cleanup.json>" }]
}
```

A plugin that only runs in the background needs no `program`. The rule's
`id` is its name, and it says what it needs from the host:

```json
"needs": { "delete": ["ended_targets", "stale_targets"], "tasks": true, "notify": true }
```

`delete` lists the kinds of folders the rule may delete, from the host's
own list (`ended_targets`, `stale_targets`, `worktrees`, `gate_pools`,
`incremental`, `trash`); `tasks` reads which Coder tasks ended; `notify`
sends a short notification. The plugin only describes the rule. The host
checks it against what it allows before it ever runs it, and does every
deletion itself with every safety check: nothing in use, unsaved, or
unpushed; no links or other volumes; nothing outside the host's cleanable
folders; a dry run and a log for every run. A rule that asks for more is
refused, and the plugin cannot be turned on.

```sh
openagents plugin install ./disk-cleanup   # installed, off
openagents plugin enable disk-cleanup      # on for this computer
openagents background list                 # its rule, with the others
openagents background run disk-cleanup --dry-run
openagents plugin disable disk-cleanup     # off again
```

In the terminal, `/plugins` shows each installed plugin on or off, and
Space turns it on or off. The [design](../background/2026-10-02-disk-cleanup-plugin.md)
says what the host enforces and why.

## Test a plugin

A plugin's tests live in `evals/`, one directory per test, each with a
`prompt.md` task and checks under `graders/`. Write them with the
interview, then run them with the plugin and without it:

```sh
cd my-plugin
openagents plugin test init               # write the tests with us, step by step
openagents plugin test run . --runs 1     # try each test once
openagents plugin test run .              # the full run: three times with, three without
```

A run writes `report.json` and `report.html` under `evals/results/`. The
result is a test result: for example, "passed 5 of 6 with it, 2 of 6
without", with a verdict of **Better**, **No clear change**, or **Worse**.
Nothing leaves your computer until you publish. The
[test specification](../extensions/evaluation.md) has the test format, the
checks, the sandbox, and how the verdict is decided.

## Publish a plugin

```sh
openagents plugin publish ./disk-cleanup   # prints its id, KEY:SLUG
openagents plugin search disk              # find published plugins
openagents plugin install disk-cleanup     # by name, or by id; it starts off
```

`publish` signs the plugin with your key: its files go to the blob
server by digest, then a NIP-EXT release (`3184`) pins its manifest and
a listing (`30184`) points at that release. A plugin's id is your public
key and its slug. Publishing the same version again changes nothing;
publishing other files under the same version refuses, so raise
`version` in `package.json`. `--fee-msat N --payout ADDRESS` puts a
per-call fee and the Lightning address (or node key) paid into the signed
release. `install` checks the release's signature and revocations and
every file's digest and size against it before it installs; like a local
install it leaves the plugin off. In the terminal, `/plugins` lists the
published plugins too, and Enter installs one.

## Publish a test result

```sh
openagents plugin test publish evals/results/<timestamp>/report.json
```

`publish` adds your result to the Gym: it releases your test set, signed by
your key, and publishes the result. Other trainers rerun your tests with
`openagents plugin test check <result id>` (in the app, **Check a result**)
to confirm or dispute it. When three trainers confirm a **Better** result
and the plugin also does better on a test set someone else wrote, Coder can
use the plugin for everyone; `openagents plugin defaults sync` writes the
plugins Coder uses on your computer.

`openagents plugin list` shows published plugins. `openagents ext` is the
older name for `openagents plugin` and still works.

## Words we use

| You read | It means | In the specifications |
| --- | --- | --- |
| Plugin | Anything you add | The plugin's parts, shipped in an extension package ([NIP-EXT](../../nips/openagents/NIP-EXT.md)) |
| Skill | Instructions the agent reads | A `skill` component |
| Workflow | A typed, step-by-step program | A program ([NIP-PRG](../../nips/openagents/NIP-PRG.md)), a `program` component |
| Knowledge | Cited reference entries | A knowledge entry ([NIP-KB](../../nips/openagents/NIP-KB.md)) |
| Wasm | Sandboxed code with one bounded operation | A Wasm guest, the `plugin` component kind |
| Tests | The tasks and checks that show whether a plugin helps | An eval suite, the `eval-suite` component |
| Test result | What the tests showed, with and without the plugin | A capability claim ([NIP-EVAL](../../nips/openagents/NIP-EVAL.md) `3189`) |

The [glossary](../glossary.md#one-vocabulary-what-you-can-add) explains
each internal term.

## Inspect contribution evidence

OpenAgents Terminal and Verse use the same read-only contribution pane. Pass
`--contribution-workbench CONFIG` to `openagents-terminal`, or pass
`--terminal --contribution-workbench CONFIG` to Verse. Keep the configuration
and selected evidence files private (mode `0600` on Unix). Paths select existing
retained records; opening a pane runs no plugin, publishes nothing, and pays
nothing.

```json
{
  "plugins": ["/absolute/path/to/reviewed-plugin-owner"],
  "knowledge": ["/absolute/path/to/knowledge-session.json"],
  "reviews": ["/absolute/path/to/signed-prospective-bundle.json"],
  "events": ["/absolute/path/to/retained-signed-event.json"],
  "documents": ["/absolute/path/to/pinned-manifest.json"],
  "operators": [],
  "evaluators": [],
  "referees": [],
  "ledger": null
}
```

Use hex public keys for the operators, evaluators, and XP referees you trust.
Select all signed results, checks, release events, quests, and awards needed to
verify a claim, and the manifest and admission documents their digests pin.
Keep signed retirement bundles beside earlier admission bundles: retirement
wins for that admission regardless of file order. A changed exact version
needs its own evidence. Local comparisons, publication, installation, actual
route invocation, independent validation, operator adoption, signed credit,
and settlement appear separately. Source text, model output, payer aliases,
and invoices are excluded.

The optional `ledger` selects an existing payment ledger opened read-only. A
settled contribution requires an exact signed release's author share in a
payout recorded as sent, with a wallet reference and a positive recorded sent
amount. The pane labels this as a local ledger claim. A payout batch's amount
is not the individual share's amount. Failed, uncertain, missing, and unrelated
payments establish no settlement; no selected receipt means unavailable.
