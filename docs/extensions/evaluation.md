# Extension evaluation

Status: target specification, revision 2 (2026-09-28). Nothing on this
page is built yet. It builds on parts that exist: the Wasm host core and
its invocation receipts (`crates/plugin`), the three evidence guests, the
local package resolver and locks (`crates/coder/src/package.rs`), the
program runtime (`crates/coder/src/runtime.rs`), the headless `coder -p`
turn (`crates/coder/src/headless.rs`), ATIF trajectories (`crates/atif`),
confinement (`crates/coder-boundary`), the Gym's gates
(`crates/gym/gates/`), NIP-EVAL reports and `3189` publications, NIP-XP
awards and the XP ledger (`crates/xp-ledger`), and the chat router
(`crates/coder/src/router.rs`). See [What is built](plugins.md#what-is-built),
[the Gym index](../gym/README.md), and
[the chat router](../coder/design/2026-09-28-chat-router.md).
Implementation is tracked in epic
[#9931](https://github.com/OpenAgentsInc/openagents/issues/9931)
([Delivery](#delivery)).

An **extension eval** measures whether an extension changes what Coder
does. It runs a set of cases against Coder with the extension admitted,
runs the same cases with the extension absent, and reports scored results
with the change between the two. A published result can be checked by
other trainers, and the people who wrote the extension and its cases earn
XP when their work is checked and when it's adopted into Coder's defaults.

There are two ways in:

- **Chat, the product path.** A person asks OpenAgents, in the one routed
  chat, what's new in the Gym, which tool to test, or to make a tool and a
  test set with them. OpenAgents asks questions, drafts the tests, runs a
  pilot on our computers or on their connected computer, shows the result
  as a card, and publishes it only when they tap **Add to the Gym**. The
  [app wireframe](../product/2026-09-28-app-wireframe.md) (revision 3)
  specifies those screens.
- **`openagents ext eval`, the engine.** The same runner, as a command
  for people working on an extension in a terminal. Chat drives this
  engine: a dispatched Coder runs it on a connected computer, and the
  hosted runner runs it on our computers.

A suite measures one extension. It is not a benchmark of Coder and it is
not a security review of the package: a suite written by an extension's
author describes intended behavior. An operator who admits someone else's
extension runs a suite they trust.

## Contents

- [Changes from the previous draft](#changes-from-the-previous-draft)
- [Terms](#terms)
- [Words on screen](#words-on-screen)
- [Quick start](#quick-start)
- [Targets](#targets)
- [Where the suite lives](#where-the-suite-lives)
- [Case format](#case-format)
- [Graders](#graders)
- [Scoring and the verdict](#scoring-and-the-verdict)
- [The run sandbox](#the-run-sandbox)
- [Where runs execute](#where-runs-execute)
- [Trust and admission](#trust-and-admission)
- [Results](#results)
- [Publishing and the Gym](#publishing-and-the-gym)
- [Checks, adoption, and credit](#checks-adoption-and-credit)
- [Chat: the product path](#chat-the-product-path)
- [Authoring a suite](#authoring-a-suite)
- [Iterate on an extension against its suite](#iterate-on-an-extension-against-its-suite)
- [The system prompts](#the-system-prompts)
- [Wire formats](#wire-formats)
- [Later](#later)
- [Reference: limits and constants](#reference-limits-and-constants)
- [What this page does not claim](#what-this-page-does-not-claim)
- [Delivery](#delivery)

## Changes from the previous draft

The first draft was written before we checked it against the codebase. The
owner asked us to bring it in line with how the code works, with our
invariants, and with the chat-first product. This revision keeps the
draft's shape (cases, graders, a baseline arm, a sandbox, a result
document, publication, an iterate loop, and an authoring interview) and
changes the following.

| Change | Previous draft | This revision | Why |
| --- | --- | --- | --- |
| Product path | A CLI with a phone button for publishing. | The routed OpenAgents chat is the product path: asking what's new, choosing or making a tool, drafting tests, running, reading the result, publishing, checking others, and credit all happen in chat, with a few dedicated sheets. The CLI is the engine chat drives. | Owner direction, 2026-09-28: people use the Gym and evals through our chat and its router. |
| Manifest | `package.toml` and `experimental.eval_dir`. | The package record `coder::package::Package` already loads (JSON), with an `eval_dir` field. | There is no `package.toml` in the repository; packages are JSON records. |
| Case version key | `schema_version = "1.0"`. | `v = "openagents.eval-case.v1"`. | Every other contract here names its schema in `v`. |
| Turn limit | `run.max_turns` (default 10). | Removed. A run is one `coder -p` turn under Coder's own bounds (shell rounds, commands, timeouts), which the run config records. `run.deadline_seconds` stays. | `coder -p` has no turn cap; one headless turn is what a run is. |
| Door pinning | Inherited environment, `--door`. | The harness sets `CODER_DOOR_URL`, `CODER_DOOR_KEY`, and `CODER_MODEL` in the child and records the door; nothing is inherited. | That is how `Agent::from_env` pins a door today. |
| Trajectory | "One step per line" document. | Each run writes an ATIF v1.8 log with `crates/atif`; graders read the rendered document. | ATIF is our trajectory format (`NIP-ATIF`). |
| Confinement | Best effort, "silent no-op elsewhere", Windows Job Objects. | The whole child runs inside `coder-boundary` (`sandbox-exec` on macOS, `bwrap` on Linux). Where neither exists, the run refuses. | `coder-boundary` refuses unsupported hosts; a silent no-op contradicts that, and Coder's plain shell is otherwise unconfined. |
| Stand-ins | Stand-ins for host entries and doors, agent responders, record and replay. | Moved to [Later](#later). | No extension can declare a host entry yet: the concept has no code. We add stand-ins with host entries. |
| `baseline` grader and `context.history` | In v1. | Moved to [Later](#later). | Both need replayed trajectories we don't produce yet; v1 compares arms directly. |
| Judge | An unnamed "judge door". | `decision` graders ask Jev through `POST /v1/systemone` (the default); a `judge` grader asks the chat model door for PASS or FAIL. | Jev is our typed selector; a prose judge is new code and says so. |
| Verdict | A keep rule written in prose. | A Gym gate, `crates/gym/gates/ext-eval-v1.json`, digested like every gate, decides **Better**, **No clear change**, or **Worse**. | The Gym already records which rule produced which verdict. |
| Suite publication | Not specified. | A suite is published as a NIP-EXT release whose package holds an `eval-suite` component. | Suites need an author, versions, and revocation; NIP-EXT has all three and needs no new kind. |
| Result publication | "Kind `3189`" with no profile. | A `3189` NIP-EVAL publication with the `oa:ext-eval:v1` profile marker, citing the suite release. | Same kind, with a marker readers filter on, as NIP-XP's run evidence does. |
| Credit | "A `gym-trial` XP award candidate". | Two NIP-XP rules, `eval-check` and `eval-adopt`, define exactly when authors, publishers, and checkers earn XP. No money. | `gym-trial` never had a definition; the owner asked for credit when work is used. |
| Hosted runs | Not specified. | A hosted runner runs suites for chat over NIP-CJ execution jobs, for catalog tools and chat-made tools only. | A phone can't run `coder -p`; IDIOT PROOF means no computer is needed for a first run. |
| Kill switch | A tenancy capability flag with a fail-open fetch. | Removed from v1. The hosted runner has its own admission switch; the CLI runs locally. | No capability-flag endpoint exists, and a fail-open client check stops nothing. |
| Names | `ext eval` was the only name. | Same CLI group. `openagents eval` stays the decision-door command it is today. | `openagents eval run|report|compare` already exists for doors. |

## Terms

| Term | Meaning |
| --- | --- |
| Extension | An installed package, or a working directory with a package record, that supplies programs, Wasm guests, or skills. The phone calls it a **tool**. |
| Suite | Every case under the extension's eval directory. The phone calls it a **test set**. |
| Case | A directory with `prompt.md`, optional `graders/`, and an optional `case.toml`. The phone calls it a **test**. |
| Grader | A scored check over a run's trajectory, final message, or created files. |
| Arm | One side of a comparison: `subject` (the extension admitted) or `baseline` (nothing admitted). |
| Run | One headless `coder -p` turn for one case in one arm. |
| Check | A rerun of a published suite against the same subject by a different trainer. The phone calls it **Check a result**. |
| Hosted runner | The OpenAgents service that runs suites on our computers for chat, over NIP-CJ execution jobs. |
| Coder defaults | The `openagents:coder-defaults` package whose releases list the extensions Coder admits for everyone. |

## Words on screen

Engineering words stay in code, the CLI, and this page. The phone uses
plain words (see the wireframe's
[Words on screen](../product/2026-09-28-app-wireframe.md#words-on-screen)).

| Here | On the phone |
| --- | --- |
| Extension | **Tool** |
| Suite | **Test set** |
| Case | **Test** |
| Subject and baseline arms | **With** the tool and **without** it |
| Suite score | **Passes 7 of 8 tests** |
| Verdict `pass`, gate keep, inconclusive, reject | **Better**, **No clear change**, **Worse** |
| Publish (`3189`) | **Add to the Gym** |
| Check | **Check a result** |
| `eval-check` and `eval-adopt` XP | **XP**, and "Coder now uses your tool" |

## Quick start

```sh
cd my-extension                        # a directory with a package record
openagents ext eval init               # interview: writes evals/<case>/
openagents ext eval init smoke --bare  # or a blank single-case template
openagents ext eval run .              # every case under ./evals/, both arms
openagents ext eval run . --runs 1 --case smoke   # a cheap pilot
openagents ext eval publish evals/results/<timestamp>/report.json
openagents ext eval check <publication event id>  # rerun someone's result
```

A run prints progress on stderr and a summary table on stdout, and writes
`<eval dir>/results/<timestamp>/` holding `report.json` (the result
document), `report.html` (a self-contained view of it), and `runs/` (each
run's ATIF log and the files it created). Nothing leaves the machine until
`publish`.

`run`, `init`, `publish`, and `check` are the commands of `openagents ext
eval`, beside the existing `ext list`. The decision-door command
`openagents eval run|report|compare` is unchanged.

## Targets

`openagents ext eval run <target>` accepts:

- A path: the extension's root directory runs every case under its eval
  directory; a case directory inside it runs that case only.
- An installed identity `<root-pubkey>:<slug>@<version>`: resolved through
  the local package store; results are written under the current
  directory.
- A published suite, `--suite <release event id>`, run against the target
  named in the suite, which is how `check` works.

A directory whose package record doesn't resolve (`Package::resolve`
refuses it) runs nothing and exits 1 with the refusal. A target that is
not owned by the operator or is writable by others is refused the same way.
Nothing is ever run with the extension silently absent.

## Where the suite lives

The eval directory is `evals/` at the extension root. `--eval-dir <dir>`
overrides it per run, and the package record's `eval_dir` field overrides
it per extension; the flag wins over the record, and the record over the
default. A value that is not one or more plain directory names below the
extension root is a usage error.

A chat-made test set has no directory on the phone: it is a draft document
(see [Chat](#chat-the-product-path)) that the hosted runner or the
connected computer writes out as this same layout before running it, so
both paths run identical bytes.

## Case format

A case is a directory containing `prompt.md` and optionally `case.toml`.
Discovery looks only beneath the eval directory, skips `.git`,
`.openagents`, `node_modules`, and `results`, never recurses into a case
directory, and runs cases in lexicographic order.

```text
evals/<case>/
├── prompt.md          TOML frontmatter between +++ fences; the body is the prompt
├── graders/
│   └── <grader>.md    TOML frontmatter; the body is the rubric or pattern
├── fixtures/          optional files copied into the run's empty workspace
└── case.toml          optional; the base document prompt.md overrides
```

`prompt.md` frontmatter keys: `v`, `name`, `description`, `tags`,
`kind`, `extensions`, `runs`, and a `[run]` table with `deadline_seconds`,
`door`, `allowed_operations`, `append_instructions`, and `env`. Any other
key is an error that names the allowed set. When both files exist,
`case.toml` is the base, `prompt.md` frontmatter overrides it, the
`prompt.md` body is the prompt, and the grader list is `case.toml`'s
`graders` followed by `graders/*.md` in name order. A grader's name is its
file name without `.md` unless its frontmatter names one. Each file is at
most 1 MiB, with at most 64 grader files per case.

| Field | Type and default | Meaning |
| --- | --- | --- |
| `v` | string, required in `case.toml` | `"openagents.eval-case.v1"`. A newer major version refuses with the version it needs. |
| `name` | string, default the directory name | The name `--case` globs match. Duplicate names are an error. |
| `description` | string | For readers. |
| `tags` | string[], `[]` | `--tag` keeps a case when any tag matches. |
| `kind` | `should-fire` (default) or `should-not-fire` | Whether the extension ought to be used on this task. The phone shows it as "a test where the tool should help" or "should stay out of the way". |
| `extensions` | string[] | Components under test as package-relative paths or identities. Default: the package at the extension root. |
| `runs` | int 1 to 10, `3` | Runs per arm. One run of a nondeterministic agent is noise. |
| `run.prompt` | string | The prompt (the `prompt.md` body). |
| `run.deadline_seconds` | int up to 1800, `300` | Wall-clock cap; the child is stopped and the run marked `timeout`. |
| `run.door` | string | The door the subject turn uses, as a name the runner's door table knows. `--door` overrides it. With neither, the runner's default door, recorded in the report. |
| `run.allowed_operations` | string[], `["read"]` | Operations the case asks for: `read`, `write`, `exec`, `network`. Only `read` is automatic; the rest need the operator's `--grant` (or the hosted runner's fixed grant, below). |
| `run.append_instructions` | string | Appended to the child's instructions, in both arms. |
| `run.env` | map, `{}` | Extra environment. Keys must match `OA_EVAL_[A-Z0-9_]*`; anything else fails the run as `env_var_rejected`. |
| `graders` | list, at least one | See [Graders](#graders). |

The template `openagents ext eval init <name> --bare` writes `prompt.md`:

```markdown
+++
v = "openagents.eval-case.v1"
kind = "should-fire"
+++

TODO: describe a task someone would give Coder
```

and `graders/criteria.md`:

```markdown
+++
type = "decision"
question = "Did the run do what the task asked?"
threshold = 0.7
+++

TODO: describe what a successful run looks like
```

A run refuses a case that still has a `TODO` line.

## Graders

Every grader has `type`, `name`, `weight` (greater than 0, default 1), and
an optional `arm`. Structural graders run locally and cost nothing;
`decision` and `judge` graders call a door and spend the caller's quota. A
grader that can't evaluate reports why and fails.

What a grader reads (`target` for `regex`, `focus` for `decision` and
`judge`):

| Value | Content |
| --- | --- |
| `last_message` (default) | Coder's final assistant text. |
| `trajectory` | The run's rendered ATIF document. A door sees the first and last 12 steps and the final message. |
| `files` | The paths of files the run created. Paths, not contents. |
| `{ file = "<path>" }` | The contents of one file in the run's workspace after the run, at most 1 MiB, inside the workspace only. Text only in v1. |

Grader types in v1:

| Type | Keys | Passes when |
| --- | --- | --- |
| `regex` | `pattern`, `flags`, `match` (`contains`, `not_contains`, `count:N`), `target` | The pattern is (or is not) found; `count:N` requires exactly N. |
| `operation_used` | `operation`, `input_match`, `min` (1), `max` | The number of matching calls in the trajectory is within `min..=max`. "Must not run" is `min = 0, max = 0`. An operation is a program step, a Wasm guest operation, or a shell command, as the ATIF `Call` records it. |
| `operation_order` | `before`, `after` | Both ran and the first `before` precedes the first `after`. |
| `file_exists` | `path` (a glob over created files), `exists` (true) | A created file matches, or none does. |
| `decision` | `question` (a Jev Score or Choice question over the focus), `threshold` | Jev's answer through `POST /v1/systemone` is at or above `threshold` on at least two of three calls. The default grader. |
| `judge` | `criteria` (the body), `focus` | The chat model door answers `PASS` on at least two of three calls and `FAIL` on none. For prose criteria a typed question can't hold. |
| `receipt` | `operation` | The Wasm guest's invocation receipt replays exactly on this host (`plugin::replay`). |

The `receipt` grader is the deterministic floor for Wasm guests: it reruns
the recorded invocation and compares outcome and fuel. It proves the same
bytes behaved the same on this engine, nothing more; pair it with an
outcome grader.

Choosing graders:

- Prefer deterministic graders for long outputs; a door's answers get
  noisy past a few thousand characters.
- Grade outcomes (a file's contents, the final message) and mechanism
  (`operation_used`). The harness warns when a case can't pass with the
  operations it asked for.
- A should-not-fire case uses `operation_used` with `max = 0` on the
  extension's operation plus an outcome grader.

### The baseline arm and subject-only graders

Each case runs in both arms by default; `--baseline off` runs the subject
arm alone and can't produce a verdict. A grader that only makes sense with
the extension present (`arm = "subject-only"`, and every `operation_used`
grader on an extension-supplied operation with no explicit `arm`) is
dropped from the baseline arm and from the score in both arms, so the
change compares like with like. It still reports, with `scored = false`.
`arm = "both"` keeps it in both arms. If every grader in a case is
subject-only, the case scores the subject arm alone and is excluded from
the change.

## Scoring and the verdict

- A **run** scores the weighted mean of its scored graders, from 0 to 1,
  and **passes** when every scored grader passes.
- A **case** scores the mean of its runs and **passes** in an arm when a
  majority of its runs pass.
- The **suite** reports cases passed per arm ("7 of 8 with the tool, 5 of
  8 without"), the mean case score per arm, and the change
  (`subject - baseline`) per case and overall. It also reports cost and
  wall time per arm.
- The **verdict** comes from the Gym gate `ext-eval-v1`
  (`crates/gym/gates/ext-eval-v1.json`), recorded by its digest like every
  gate ([Gate digests](../gym/gate-digests.md)). It keeps (**Better**)
  only when the subject arm passes at least as many cases as the baseline,
  passes every should-not-fire case the baseline passes, and improves the
  mean score, cost, or time by more than the spread between repeats of
  the same arm. It rejects (**Worse**) when the subject arm passes fewer
  cases or loses a should-not-fire case. Everything else is
  **inconclusive** (**No clear change**). The spread is measured on this
  run's repeats; with `runs = 1` every verdict is inconclusive.

## The run sandbox

Each run spawns one `coder -p` turn inside a new directory
`<tmp>/oa-eval-XXXXXX`:

```text
home/
  cwd/           the child's working directory: fixtures and a repository marker
  .openagents/   a fresh personal directory: throwaway keys, empty stores
out/             the ATIF log, grader answers, and door calls
tmp/             a private temp directory, mode 0700
```

- The child runs inside `coder-boundary` with `home/` and `tmp/` writable
  and everything else read-only; network is limited to the pinned door
  unless the case was granted `network`. Where `coder-boundary` has no
  backend (anything but macOS and Linux), the run refuses with
  `unconfined_host`.
- `HOME`, `OPENAGENTS_HOME`, `TMPDIR`, `TMP`, and `CODER_TRACE_DIR` point
  inside the directory; `XDG_*` and every `CODER_*` variable of the
  operator's shell are removed. The harness then sets `CODER_DOOR_URL`,
  `CODER_DOOR_KEY`, and `CODER_MODEL` for the pinned door, and
  `CODER_PROGRAMS` and `CODER_PROGRAM_EFFECTS` for the subject arm's
  admitted programs. The door key is never written to the trajectory,
  the report, or `out/`.
- The workspace starts with the case's fixtures and nothing else. The
  default grant is read-only. `allowed_operations` asks for more; only the
  operator's `--grant` admits `write`, `exec`, or `network`.
- The subject arm admits the extension's components: its programs through
  `CODER_PROGRAMS` under the runtime's ceilings, its Wasm guests through
  the program `module` step, and its skills as digested guidance appended
  to the child's instructions. The baseline arm admits none of them.
- The run lock (`coder::package::HeldLock`) pins every admitted component
  digest for the whole run, so a result is a claim about exact bytes. The
  harness never edits the extension.
- `--keep-temp` keeps the directory and prints its path; by default it is
  deleted after graders read it, and `out/` is copied into the results.

`SIGINT` (exit 130) and `SIGTERM` (exit 143) stop every live child after a
short grace period; a second signal exits at once.

## Where runs execute

| Where | Who starts it | What it may run | Evaluator key |
| --- | --- | --- | --- |
| The operator's computer, `openagents ext eval run` | The operator in a terminal | Any extension the operator trusts, with any grant they pass | The operator's Verse world key |
| A connected computer, from chat | A tap on **Run on Studio Mac** in chat, which sends Coder a NIP-HOST task that runs `openagents ext eval run` | The same as the operator's computer; the grant is read and write in the sandbox only, never `exec` or `network`, unless the person approves it on the computer | The computer's world key |
| The hosted runner, from chat | A tap on **Start the test** in chat, which sends a NIP-CJ execution job | Only catalog tools (tools in the OpenAgents catalog or in Coder defaults) and chat-made tools whose components are skills and choices of catalog tools. No setup programs, no `exec` or `network`, at most 8 cases, 3 runs, and a daily quota per trainer. | The hosted runner's key, with the requesting trainer named |

Every path runs the same crate (`crates/ext-eval`), writes the same
report, and publishes the same way. A chat request never runs anything
until the person taps; the tap sends a request signed by their device.

## Trust and admission

Evaluation sits on the [package trust ladder](packages.md#trust-is-several-independent-decisions):
known, verified, installed, enabled, granted, admitted.

- A directory target the operator hasn't trusted asks once, in plain
  words, that the run will load the extension's components and run them
  as the operator, and that this is not a security check. It defaults to
  no. `--trust` answers yes for scripts; without a terminal, an untrusted
  directory refuses.
- An installed identity is already trusted by installation.
- Trust covers loading and running. Every grant beyond `read` still needs
  `--grant`.
- The hosted runner trusts only the catalog and the chat-made components
  described above; it refuses anything else as `not_admitted` before any
  run.

## Results

`report.json` is an `openagents.eval-report.v1` document (NIP-EVAL) with
the extension-evaluation profile in `meta.ext_eval`:

- `suite`: the suite ArtifactRef (its cases digested in order), and the
  suite's release EventRef when it was published.
- `subject` and `baseline`: the admitted lock, the pinned door, the
  decision and judge doors, and the run configuration.
- `runs`: per case, per arm, per attempt: each grader's verdict with its
  explanation, the ATIF log's ArtifactRef, and the created files list.
- `coverage`: planned, attempted, completed, refused, failed, cancelled,
  and unknown counts per arm.
- `measurements`: case score and pass per arm, suite cases passed and mean
  score per arm, the change per case and overall, and cost and time per
  arm.
- `verdict`: `pass` (Better), `fail` (Worse), or `inconclusive`, with the
  gate's digest.
- `meta.ext_eval`: `{kind: "should-fire" | "should-not-fire"}` per case,
  the headline counts, and the requester for hosted runs.

`report.html` is a self-contained view: the headline, each case with both
arms, each grader's answer, and links to the trajectories.

A run's errors become grader failures with a reason: `timeout`,
`refused`, `cost_ceiling`, `auth_failed`, `env_var_rejected`,
`unconfined_host`. A suite that couldn't finish writes `partial` with the
reason.

`--json [path]` writes the result document instead of the summary.
Exit codes: `0` Better or a clean single-arm run, `1` Worse,
inconclusive, or a load failure, `2` partial, `64` invalid usage, and
`130` or `143` on a signal.

## Publishing and the Gym

`openagents ext eval publish` (on the phone, **Add to the Gym**) makes a
result public. It does two things, each only once:

1. **Publish the suite**, if it isn't published yet: a NIP-EXT release
   (`3184`), signed by the suite author's key, of a package holding one
   `eval-suite` component (the cases and graders as listed files). A
   suite that ships inside the extension's own package is published with
   that package.
2. **Publish the result**: a NIP-EVAL `3189` publication with the
   `oa:ext-eval:v1` marker, signed by the evaluator, citing the suite
   release, the subject release, and for a hosted run the trainer's signed
   request.

What becomes public: the suite (every case, grader, and fixture), the
subject and baseline locks, the report, and the trainer name of the
evaluator and author. Trajectories stay private in v1 (the report carries
their digests); publishing scrubbed trajectories through NIP-ATIF comes
[later](#later). The phone says this in one sentence before the tap.

The Gym lists published results for chat to answer from: a reader fetches
`3189` events with the `oa:ext-eval:v1` marker, verifies each against its
report and suite digests, and groups them by extension. Nothing is ranked
across different suites.

## Checks, adoption, and credit

**A check** is a rerun of a published suite against the same subject
release by a trainer who is not the evaluator. It publishes its own `3189`
that cites the original with an `e` tag. It confirms the original when its
verdict matches; it disputes it otherwise. Both outcomes stay visible.

**Adoption** makes a tool part of Coder for everyone: an OpenAgents
operator issues an `openagents.eval-admission.v1` decision (NIP-EVAL)
citing the reports, then publishes a new release of the
`openagents:coder-defaults` package that depends on the extension's
release. A tool is a candidate for adoption when its result is **Better**
and at least three checks by distinct trainers confirmed it. Adoption is
an operator decision, never automatic.

**Credit is XP and your name, not money.** Under the two NIP-XP rules this
revision adds (see [NIP-XP](../../nips/openagents/NIP-XP.md#eval-check)):

| Rule | When it's accepted | Who earns | Size (quest-defined) |
| --- | --- | --- | --- |
| `eval-check` | A check confirms a published result: a different trainer, the same suite and subject, a matching verdict, published after the original, inside the season. | The checker; the original evaluator; the suite's author. Each at most once per suite version per season. | 50, 25, 25 in the first quests |
| `eval-adopt` | A `coder-defaults` release depends on the extension, citing an admission that cites the extension's reports. | The extension's author; the author of the suite whose results were cited; each evaluator of a cited, confirmed result. Once per extension release. | 200, 100, 50 in the first quests |

"Used" means exactly these two events: someone else reran your test set
and got your result, or your tool was adopted into Coder's defaults. A
run, a publish, a view, or a download earns nothing. The OpenAgents referee
signs awards after checking each rule against the signed events; a reader
recomputes them. XP can't be spent, transferred, or converted. Nothing
here pays sats or credits: paid rewards need a funded quest purse under
NIP-MKT and NIP-LAB, which doesn't exist, and the app never promises one.

## Chat: the product path

Everything above is reachable from the one routed OpenAgents chat. The
router (Jev's typed route question, never word matching) adds routes for
Gym and eval turns; see [the chat router](../coder/design/2026-09-28-chat-router.md)
and [the wireframe](../product/2026-09-28-app-wireframe.md). In short:

| The person says | The route | What chat shows |
| --- | --- | --- |
| "What's new in the Gym?", "What are people working on?" | `gym.news` | A grounded answer from the Gym knowledge source (published results, suites, checks waiting, adoptions, the app's changelog, and product notes), every item with its source, and at most one offer. |
| "Test Project map on Coder", "Which tool should I try?" | `eval.run` | A tool card with its latest result and **Start the test**. |
| "Help me make a tool that…", "Write tests for my tool" | `eval.author` | The authoring interview, one question per turn, with the draft as a card. |
| "Check someone's result" | `eval.check` | A check card with **Run the check**. |
| "How did my test do?", "Should I add it?" | `eval.result` | A result card, **See details**, and **Add to the Gym**. |
| "What have I earned?", "Did anyone check my tests?" | `eval.credit` | A credit card from the XP ledger: pending and confirmed awards, and who checked what. |

Rules chat keeps:

1. **Talk leads to a tap.** Chat never starts a run, spends quota,
   publishes, or runs a check by itself. It offers the step as the app's
   own control, and the tap sends the signed request.
2. **Numbers come from records.** Every score, count, and news item in a
   reply comes from a verified record the router read, with its source;
   the model never states a result it wasn't given. With no record, chat
   says so.
3. **The draft is the person's.** A chat-made test set is a draft the
   phone keeps and shows as a card; each interview gate is a tap
   (**Looks good**, **Change it**); nothing is public until **Add to the
   Gym**.
4. **Plain words.** Chat uses the phone's words (tool, test, test set,
   with and without); the engineering words stay here.

What "make a tool" means in chat in v1: a tool made in chat is a skill
(plain-language instructions Coder follows) that may also turn on catalog
tools such as Project map. Tools with new code (Wasm guests, programs) are
made with Coder on a connected computer or in a terminal, and are then
tested the same way.

## Authoring a suite

The interview is one typed state machine (`crates/ext-eval` `author`)
with two drivers: `openagents ext eval init` in a terminal (a Coder
session on the operator's machine) and the chat's `eval.author` route
(the chat worker, with the draft kept on the phone). The steps, each gated
on explicit approval:

0. **Gate.** Confirm the target is a real extension (or, in chat, which
   tool the person means, or that they're making one). Stop on an error
   rather than guess.
1. **Read the tool.** Its components and their own words. Say what it's
   for, what it does on its own, and what it leaves to the person.
2. **Define quality.** Ask what a good run looks like and what a failure
   looks like. The graders follow from the answer.
3. **Propose tests.** At least 4 should-fire and 1 to 2 should-not-fire
   tests, each named for a real task shape. Wait for approval.
4. **Propose graders.** Every test gets an outcome grader;
   `operation_used` graders confirm the tool was reached. Default to
   `decision` graders over prose judges. Wait for approval.
5. **Pilot.** Run one run per arm on the cheapest path, read every run,
   fix the suite, and repeat until clean. In chat, the pilot is a tap
   (**Try it once**) and its result is a card.
6. **Estimate and confirm.** Quote the full run's size (tests × runs ×
   arms) and, on the operator's computer, its cost; get approval.
7. **Write.** Write the cases (terminal) or keep the draft (chat), and
   say how to run it.

Floor invariants the machine enforces, whatever the model proposes: at
least one should-not-fire test, at least one outcome grader per test,
`runs` of at least 3 for a scored run, the baseline arm on, and no test
prompt that names the operation to call. `--bare` skips the interview and
writes the template.

## Iterate on an extension against its suite

1. Work on the development cases. A held-out set, when a suite has one, is
   run once, to confirm a change that already passed.
2. Change one thing per round and record each round (change, cases passed
   per arm, change, verdict, report path), including rounds that scored
   worse.
3. The noise floor is the spread between repeats. A change inside it is
   not a signal; rerun before keeping or reverting.
4. Keep a change only when the gate says **Better**. Drop a component the
   graders never see help from the default set, then from the package.
5. Read failing runs before editing, and fix the extension, not the
   grader, unless the grader is wrong.

A round's numbers come from `report.json`, never from memory of an
earlier summary.

## The system prompts

The braces are substitution points.

### The interview prompt

```text
# Tool test-set interview

You are helping ${person} write a test set for the tool at ${tool}. You
speak as OpenAgents ("we") in plain words: tool, test, test set, with and
without the tool.

## Rules

- You may read the tool. You never edit it.
- One step per turn. At every gate (the tool's purpose, the test list, the
  checks for each test, the size of the full run) stop and wait for an
  explicit yes. Anything else is a change request: fix and ask again.
- Keep the floor: at least one test where the tool should stay out of the
  way, at least one check of the outcome per test, three runs per scored
  run, and a run without the tool to compare against.
- A check describes something we can observe: Coder's last message, the
  files it made, or the steps it took. If it can't be observed, it isn't
  a check.
- Never write a test that tells Coder which tool to use. A test is a task;
  whether the tool helps is what we measure.
- You propose; the app shows the draft and the person decides. Never say a
  test ran unless the app gave you its result.

## Steps

0. Confirm which tool this is, or that we're making one. Stop on errors.
1. Say what the tool is for, what it does, and what it doesn't.
2. Ask what a good run and a failed run look like.
3. Propose 4 to 6 tests where the tool should help and 1 or 2 where it
   shouldn't. Wait for yes.
4. Propose the checks for each test. Wait for yes.
5. Offer a one-run try. Read its result with the person and fix the tests.
6. Say how big the full run is. Wait for yes.
7. Finish: the test set is ready to run.
```

### The judge prompts

The `judge` grader's system prompt:

```text
You are a strict, terse evaluation judge for agent runs.
```

Its user prompt:

```text
## Run record
${trajectory_or_output}

## Criterion
${criteria}

Answer only with "PASS" or "FAIL".
```

A `decision` grader sends the focus as the Jev `state` with the grader's
typed `question` to `POST /v1/systemone` and compares the returned
probability with `threshold`.

## Wire formats

Every wire-level item maps onto an existing OpenAgents NIP; this revision
adds profiles and rules, and no new kinds.

| What | NIP | Record |
| --- | --- | --- |
| A suite's cases and graders | [NIP-EVAL](../../nips/openagents/NIP-EVAL.md#extension-evaluation-profile) | `openagents.eval-suite.v1` with the case format above as listed files. |
| A published suite | [NIP-EXT](../../nips/openagents/NIP-EXT.md#component-types-and-operation-descriptors) | A `3184` release whose manifest holds an `eval-suite` component. Revoked with `3185`. |
| A result or a check | NIP-EVAL | A `3189` publication with markers `oa:eval:v1` and `oa:ext-eval:v1`, `e` tags for the suite release, the subject release, and the checked publication. |
| A run's trajectory | [NIP-ATIF](../../nips/openagents/NIP-ATIF.md) | ATIF v1.8 bytes referenced by digest from the report; public `3198` publication later. |
| A hosted run | [NIP-CJ](../../nips/openagents/NIP-CJ.md#execution-jobs) | An execution job (`25920`) whose target is the `ext-eval` program, signed by the trainer's device; results `26920`, progress `27020`. |
| Chat cards and offers | [NIP-CJ](../../nips/openagents/NIP-CJ.md#conversation-jobs) | Conversation feedback `card`, offers `start_eval`, `publish_eval`, and `open_screen` for the Gym sheets, and the request's bounded `draft`. |
| Adoption | NIP-EVAL, NIP-EXT | An `openagents.eval-admission.v1` decision and a `coder-defaults` release depending on the extension. |
| Credit | [NIP-XP](../../nips/openagents/NIP-XP.md#eval-check) | `eval-check` and `eval-adopt` quests and awards; the ledger in `crates/xp-ledger`. |

## Later

These are specified in outline and deliberately not in v1:

- **The Gym in the Verse as a social place.** The Gym building keeps its
  boards for reviewing results. Later, trainers' agents in the Gym compare
  notes with each other over chat: an agent reads another's published
  suites and results, proposes a joint check, or explains why a tool
  helped one repository and not another, with every claim sourced to
  published records. Chat stays the place where work is started.
- **Stand-ins for host entries and decision doors**, including
  model-backed responders and recorded replays, once extensions can
  declare host entries.
- **The `baseline` grader** (judge a trajectory against a pinned
  baseline trajectory) and **history cases** (resume a pinned trajectory
  and evaluate the next turn).
- **Public trajectories** for published results through NIP-ATIF `3198`
  and `3199`, scrubbed like the Gym's trace bundles.
- **Held-out cases** the author keeps private until a confirmation run.
- **Paid rewards**, only through funded quest purses, never through XP.
- **A remote switch** for the CLI.

## Reference: limits and constants

| Bound | Value |
| --- | --- |
| `runs` | 1 to 10, default 3 |
| `deadline_seconds` | up to 1800, default 300 |
| `--concurrency` | 1 to 8, default 1 |
| Grader files per case | 64 |
| Case file size | 1 MiB |
| File-focus read | 1 MiB |
| Door votes | 3; `decision` needs 2, `judge` needs 2 and no `FAIL` |
| Hosted runner suite | at most 8 cases, 3 runs, 2 arms |
| Chat draft | at most 64 KiB |

## What this page does not claim

- A passing suite doesn't admit an extension anywhere; adoption is a
  separate operator decision.
- A check through the hosted runner confirms the result reproduces on our
  runner; it doesn't guard against a fault in the runner itself.
- A `receipt` pass proves the same bytes behaved the same on this engine,
  not that they are correct.
- A suite written by a tool's author describes intended behavior; it is
  not independent evidence.
- The interview's approvals are the person's decisions; answering yes
  without reading forfeits them.

## Delivery

Epic [#9931](https://github.com/OpenAgentsInc/openagents/issues/9931)
splits this page into parallel issues, each owning its files:

| Wave | Issue | Scope |
| --- | --- | --- |
| 1 | [#9932](https://github.com/OpenAgentsInc/openagents/issues/9932) | Wire formats in the NIPs and `crates/nostr` |
| 1 | [#9933](https://github.com/OpenAgentsInc/openagents/issues/9933) | The engine: cases, graders, scoring, the gate, the report |
| 1, 2 | [#9936](https://github.com/OpenAgentsInc/openagents/issues/9936) | Chat routes, the Gym knowledge source, cards, and offers |
| 1 | [#9940](https://github.com/OpenAgentsInc/openagents/issues/9940) | OpenAgents Mockup to wireframe revision 3 |
| 2 | [#9934](https://github.com/OpenAgentsInc/openagents/issues/9934) | The runner, the sandbox, and `openagents ext eval` |
| 2 | [#9937](https://github.com/OpenAgentsInc/openagents/issues/9937) | The authoring interview, in a terminal and in chat |
| 2 | [#9938](https://github.com/OpenAgentsInc/openagents/issues/9938) | Credit: the referee, the ledger, and adoption |
| 2 | [#9935](https://github.com/OpenAgentsInc/openagents/issues/9935) | The hosted runner and the starter test sets |
| 3 | [#9939](https://github.com/OpenAgentsInc/openagents/issues/9939) | The phone: cards, sheets, the hub, and the first run |
| 3 | [#9941](https://github.com/OpenAgentsInc/openagents/issues/9941) | Docs, the lo-fi round, and end-to-end verification before build 21 |

[#9942](https://github.com/OpenAgentsInc/openagents/issues/9942) holds
[Later](#later)'s Gym in the Verse. The
[launch roadmap](../roadmap/2026-09-29-launch-roadmap.md) tracks the epic
as milestone M10.
