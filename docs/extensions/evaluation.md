# Extension evaluation

Status: target specification. Nothing on this page is built. The Wasm
host core, the evidence guests, the package resolver, and the Gym
measurement plane it builds on exist; see
[What is built](plugins.md#what-is-built) and
[the Gym index](../gym/README.md).

`openagents ext eval` measures whether an extension changes what the
agent does. It runs a suite of cases against the agent with the
extension admitted, optionally runs the same suite with the extension
absent, and reports scored results. `openagents ext eval init` writes
a suite: in a terminal it runs an authoring interview that reads the
extension, sources realistic inputs, proposes graders, pilots the
suite, and writes the case files; with `--bare` it writes a blank
template instead.

This page specifies the case format, the graders, the stand-ins for
host entries and doors, the per-run sandbox, the results document,
publication into the Gym, and the loop an author runs to improve an
extension against its own suite.

A suite measures one extension. It is not a benchmark of the agent and
it is not a security review of the package: a bundled suite is written
by the package's author, and its verdict describes intended behavior.
An organization that admits a third-party extension runs a suite it
wrote itself.

## Terms

| Term | Meaning |
| --- | --- |
| Extension | An installed package or a working directory that resolves to one: programs, Wasm guests, skills, descriptors. |
| Suite | Every case under the extension's eval directory. |
| Case | A directory holding `prompt.md`, optional `graders/`, and an optional `case.toml`. |
| Grader | A scored check over a run's trace, output, or produced files. |
| Arm | One side of a comparison: `subject` (extension admitted) or `baseline` (no extension). |
| Stand-in | A harness-owned replacement for a host entry or decision door the extension declares. |
| Run | One spawned headless agent turn for one case in one arm. |

The phone app calls an extension a **tool** and a case a **practice
task**. Those words stay on the product surface; this document uses the
engineering terms.

## Quick start

```sh
cd my-extension                      # a directory with package.toml
openagents ext eval init             # interview: writes evals/<case>/
openagents ext eval init smoke --bare    # or: a blank single-case template
openagents ext eval .                # run every case under ./evals/
openagents ext eval . --runs 1 --baseline without --no-publish   # cheap pilot
```

A run prints progress lines on stderr and a summary table on stdout,
and writes `<eval dir>/results/<timestamp>/` holding `report.json`
(the versioned result document) and `report.html` (a self-contained
view of it). With `--publish`, the report is signed by the runner's
world key and sent to the configured relays as an evaluation
publication; the Gym lists it for other trainers to check.

## Targets

`openagents ext eval <target>` accepts:

- A path: the extension's root directory runs every case under its
  eval directory; a case file or case directory inside an extension
  you control evaluates that extension and runs only the named case.
- An installed identity `package@catalog`: resolves through the
  installation lock and turns the baseline arm on by default.
- Omitted: the current directory.

A target outside the operator's trust set, and a target whose manifest
is not owned or is other-writable, resolves to no extension: the run
scans the named directory, the subject arm runs with nothing admitted,
and the header says why. Target the extension root instead.

## Where the suite lives

The eval directory is `evals/` at the extension root by default.
`--eval-dir <dir>` overrides it per run, and the package manifest key
`experimental.eval_dir` overrides it per extension. Precedence is
flag, then manifest, then the default. A flag value that is not one or
more plain directory names below the extension root is a usage error;
a manifest value of the wrong type warns once and falls back to
`evals/`.

Discovery, the results directory, the no-cases hint, and `eval init`'s
output all follow the directory in effect. For an installed-package
target, results are written under the current directory.

## Case format

A case is a directory containing `prompt.md` and/or `case.toml`.
Discovery recognizes case directories only beneath the eval directory,
skips `node_modules`, `.git`, `.openagents`, and `results`, never
recurses into a case directory, and runs cases in lexicographic order.

```text
evals/<case>/
├── prompt.md          frontmatter -> case fields; body -> the prompt
├── graders/
│   └── <grader>.md    frontmatter -> grader fields; body -> rubric or pattern
└── case.toml          optional - carries what prompt.md cannot (context.*)
```

`prompt.md` frontmatter is a TOML block between `+++` fences. Its
top-level keys are `schema_version`, `name`, `description`, `tags`,
`extensions`, `runs`, and `expected_outcome`; its `run` keys are
`door`, `max_turns`, `deadline_seconds`, `allowed_operations`,
`append_system_prompt`, and `env`. Any other key is an error that
names the allowed set. `context.*` fields (`setup_program`,
`history`, `snapshots`) live only in `case.toml`, which then must
carry `schema_version` and `name`.

When both files exist, `case.toml` is the base document, `prompt.md`
frontmatter overrides it, the `prompt.md` body is the prompt, and the
grader list is `case.toml`'s `graders` followed by `graders/*.md` in
name order. A grader's name is its filename without `.md` unless its
frontmatter names one. Limits: each file at most 1 MiB, at most 256
grader files per case.

`case.toml` fields:

| Field | Type and default | Meaning |
| --- | --- | --- |
| `schema_version` | string, required | `"1.0"`. Only the major version is checked; a case declaring a newer major fails with the version it needs. |
| `name` | string, required | The name `--case` globs match. Duplicate names warn. |
| `description` | string | For readers; unused at run time. |
| `tags` | string[], `[]` | `--tag` keeps a case when any tag matches. |
| `extensions` | string[] | Components under test as package-relative paths or installed identities. Default: the nearest ancestor containing `package.toml`. Every entry resolves under the containment root and passes the ownership check. |
| `runs` | int 1-50, `3` | Runs per arm. One run on a nondeterministic agent is noise. |
| `expected_outcome` | string | For readers; unused at run time. |
| `context.setup_program` | path in the case dir | A program document run in the empty sandbox workspace before the agent starts, when the operator passes `--setup`. |
| `context.history` | path in the case dir | A pinned trajectory to resume from; the case's prompt is the next user turn. Replay the known-good prefix, evaluate turn *n*. A history case runs the subject arm only. |
| `context.snapshots` | string[], `[]` | Extra directories the run may read as snapshots, staying inside the case dir. Read-only; nothing in them loads as an extension component. |
| `run.prompt` | string | The prompt (prose layout: the `prompt.md` body). |
| `run.max_turns` | int <= 200, `10` | Turn cap. An exhausted cap is a run error and lowers the score; set it generously. |
| `run.deadline_seconds` | int <= 3600, `300` | Wall-clock cap; the run is stopped and marked timed out. |
| `run.door` | string | The generation door the agent under test uses. `--door` overrides it. With neither, the child picks its configured default and the result records that none was pinned. |
| `run.allowed_operations` | string[], `[]` | Operations the case asks for. Read-only operations are granted automatically; anything with effects needs the operator's `--grant`. |
| `run.append_system_prompt` | string | Appended to the child's system prompt. |
| `run.env` | map, `{}` | Extra environment for the child. Keys must match `OA_EVAL_[A-Z0-9_]*`; anything else fails the run. |
| `graders` | list, >= 1 | The grader table below. |

The template `openagents ext eval init <name> --bare` writes:

```markdown
+++
max_turns = 10
allowed_operations = ["read", "find", "search"]
+++

TODO: describe what the agent should do
```

and `graders/criteria.md`:

```markdown
+++
type = "judge"
weight = 1
+++

TODO: describe what a successful run looks like
```

A run refuses a case still carrying a TODO line.

## Graders

Every grader has `type`, `name`, `weight` (> 0, default 1; there is no
`weight = 0`), and optional `arm`. Structural graders run locally;
`judge`, `decision`, and `baseline` graders ask a door and spend the
caller's quota. A grader that fails to evaluate reports its reason and
fails the check.

What a grader reads (`target` for `regex`, `focus` for `judge`):

| Value | Content |
| --- | --- |
| `last_message` (default) | The agent's final assistant text. |
| `trace` | The run's trajectory document, one step per line. The judge sees the first and last 12 steps. |
| `files` | The list of file paths the run created - paths, not contents; pre-existing and merely modified files do not appear. |
| `{ source = "file", path = "<path>" }` | The contents of one file in the sandbox workspace after the run (at most 10 MiB, inside the workspace only). Text decodes as UTF-8; an image (PNG, JPEG, GIF, WebP by content) is shown to the judge as an image; other binaries are refused with a note to render or extract them. |
| `mock_calls` | The run's calls to stand-ins: entry, input, the stand-in's answer, and whether it was a result, an error, or an abort. |

Grader types:

| Type | Keys | Passes when |
| --- | --- | --- |
| `regex` | `pattern`, `flags`, `match` (`contains`, `not_contains`, `count:N`), `target` | The pattern is (or is not) found in the target; `count:N` requires exactly N. |
| `operation_used` | `operation`, `input_match`, `min` (default 1), `max` | The count of matching calls in the trace falls in `min..max`. "Must not run" is `min = 0, max = 0`. |
| `operation_order` | `before`, `after` (each a name or `{ operation, input_match }`) | Both ran and the first `before` precedes the first `after`. |
| `file_exists` | `path` (glob over created files), `exists` (default true) | A created file matches - or none does, with `exists = false`. |
| `decision` | `question` (a Score or Choice question over the focus), `threshold` | The decision door answers at or above `threshold` on each of three independent calls. |
| `judge` | `criteria` (the `.md` body in prose), `focus` | A judge answers `PASS` on a strict majority of three calls. |
| `baseline` | `baseline_trace` (a trajectory in the case dir), `criteria` | The judge finds the new trajectory satisfies the criterion at least as well as the baseline. |
| `receipt` | `operation` | The extension's guest produced an invocation receipt that replays exactly on this host. |

The `receipt` grader is the deterministic floor for Wasm guests: it
reruns the recorded invocation and compares outcome and fuel. It says
the bytes behaved identically on this engine, nothing more - pair it
with an outcome grader.

A `decision` grader carries a typed question in its frontmatter and
scores the focus through `POST /v1/systemone`: typed, cheap, and
reproducible where a rubric wants a judgment-shaped answer. Use `judge`
for prose criteria the typed questions cannot express.

Choosing graders:

- Prefer deterministic graders for long artifacts; judge answers get
  noisy past a few thousand characters, and the harness notes
  `long file` in the explanation above that size.
- Grade outcomes (a file's contents, the final message) plus mechanism
  (`operation_used` on the trace). A grader that checks a file the run
  creates only passes when the case allows an operation that can
  create it; the harness prints an advisory when a case cannot pass
  with the operations it asked for.
- Assert that a check ran by pairing the file grader with
  `operation_used` and `input_match` on the command.

### The baseline arm and subject-only graders

Under `--baseline without` each case runs twice: with the extension
admitted and with nothing admitted. Graders that only make sense with
the extension present - `arm = "subject-only"`, and every
`operation_used` grader on an extension-supplied operation with no
explicit `arm` - drop out of the baseline arm and out of the score in
both arms, so the delta compares like for like. They still report as a
fired indicator with `subject_only` and `scored = false`, unless every
grader is subject-only, in which case they score normally. Set
`arm = "both"` to keep one in both arms. A plain single-arm run scores
them, so a suite's absolute score can differ between modes.

## Stand-ins for host entries and doors

An extension whose programs call host entries or decision doors is
evaluated without the real service: put one Markdown file per
operation under `<eval dir>/mocks/<entry>/<operation>.md`, or under
`<case>/mocks/` for one case. `<entry>` is the name the manifest
declares. The harness registers a stand-in under the entry's own name;
the real entry never starts, its stand-in's operations are granted
automatically, and the entry's other operations are denied. An entry
the extension declares but the suite does not stand in gets an empty
stand-in and its operations are absent for the run;
`--allow-real-entries` starts the real one instead, and `--mocks off`
runs against every real entry, both billed to the operator and outside
the sandbox's confinement.

- A bare body is the canned result. `{{input.<field>}}` inserts a
  field of the call; `{{file:<path>}}` inserts a file beside the
  stand-in, where the `{input.x}` part is a plain file-name segment.
- Frontmatter `expect` maps dotted input paths (a decimal segment
  indexes an array) to a type name, a bounded `/regex/` (literals,
  `.`, escapes, character classes, quantifiers on single atoms,
  optional `^`/`$` and the `i`/`s` flags - no groups, alternation,
  backreferences, or lookaround), a literal, or a list of allowed
  literals. A call that violates it aborts the run: score 0, graders
  skipped, reported as `aborted` with the entry, operation, and
  reason. The guard is judged on the call the agent emitted and again
  on what the stand-in received, so an intervening hook that rewrites
  input does not exempt the agent's own call.
- `error = true` returns the body as an operation error the extension
  should handle.
- `type = "agent"` puts a small model behind the stand-in through the
  judge door (the harness makes the call - nothing inside the sandbox
  holds a credential), with `abort_when` listing the only conditions
  it may abort on, written about observable output. Cost stays
  bounded: at most 4 agent-answered calls per `max_turns` per run.

An entry with no operation list in its manifest still stands in: its
operations carry permissive schemas and no descriptions, and the
harness says so once.

Every stand-in call is recorded under the result directory's
`mock-calls.jsonl`. To freeze a responder's output for the future, run
with `--mocks record`: the harness writes `.replay/<entry>/` next to
the stand-ins, one JSON file per call named by the SHA-256 of its
operation and input, plus `ADOPT.txt` naming the command that copies a
recording into the suite. A recording in place takes precedence over
the responder: the call replays verbatim and no model is spent. Change
a responder or the guard and the harness recomputes the digest and
marks stale recordings for regeneration. The suite carries recordings;
the developer host never needs the responder model again.

## The run sandbox

Each run spawns one headless agent turn - the same `coder -p` path the
CLI uses - inside a throwaway directory `<tmp>/oa-eval-XXXXXX`:

```text
home/
  cwd/        the child run's working directory (never inside evals/)
  .openagents/   a fresh personal directory: throwaway world key, empty stores
config/       the pinned run config the run lock records
out/          the trajectory document, mock-calls.jsonl, raw judge answers
tmp/          a private temp directory, mode 0700
```

Isolation rules:

- `HOME`, `OPENAGENTS_HOME`, `TMPDIR`, and `TMP` point inside the
  throwaway directory; `XDG_*` variables are filtered so the child
  cannot reach the person's real stores. `OPENAGENTS_API_KEY` and the
  run's credential are injected after the setup program completes and
  removed before the graders read the directory; the trajectory never
  carries them. The subject door is pinned explicitly rather than
  inherited through the environment.
- The workspace starts empty: an inert repository marker at its root
  and the case's fixtures, nothing more.
- The run's default grant is read-only. The case's
  `allowed_operations` asks; the operator's `--grant` list is the only
  way an operation with effects - write, execute, network, a decision
  door - is admitted. A case that needs `exec` does not get it by
  asking.
- `context.setup_program` runs inside the sandbox before the agent
  starts, only when the operator passes `--setup`. It runs as the
  operator and is extension-authored content; do not pass `--setup`
  for an extension you have not reviewed.
- The child's shell is confined where the host supports it
  (`sandbox-exec` on macOS, the `bwrap` profile on Linux, Job Object
  limits on Windows; silent no-op elsewhere). The child's real
  filesystem is confined; its program and extension content are not.
- The run lock pins every component digest admitted, so a run is a
  claim about exact bytes. `openagents ext eval` never rewrites the
  extension; the interview's pilot is the same.
- `--keep-temp` moves the run's `home/` and `tmp/` into `sealed/` as
  mode-000 directories under a read-only tree, and prints the
  `chmod 700` needed to inspect them. The raw judge answers live
  beside the result under `out/` regardless.

Signals cancel: `SIGINT` (exit 130) and `SIGTERM` (exit 143) stop every
live child after a short grace period; a second signal exits at once.

`openagents ext eval` is for extensions you trust to run on this
machine. An extension's programs, setup programs, hooks, and declared
entries run as you: confinement narrows what their operations can
touch, and it is not a complete guarantee.

## Trust and admission

An eval run sits on the same ladder as any other component use (see
[packages](packages.md#trust-is-several-independent-decisions)): known,
verified, installed, enabled, granted, admitted. The harness loads
only components that are installed and enabled, and admits them only
after the operator's trust decision:

- A directory target that is not already trusted asks interactively:
  the prompt says the run will admit the extension's components and
  declared entries on this machine as you, and that it is not a
  security check. It defaults to no.
- `--trust-extension` asserts the same thing for CI. In a
  noninteractive shell, `--json`, or `CI`, an untrusted directory
  refuses without it.
- An installed `package@catalog` target is already trusted by
  installation; the prompt does not appear.
- The trust decision covers loading and running the extension. Every
  grant beyond read-only still needs `--grant`, and `--setup` still
  needs its own flag.

## Results

The run's result document is a versioned `openagents.eval-report.v1`
shape - the same contract a Gym suite publication carries - so the
CLI's output and the measured record are one document:

- `suite`: the eval directory's ArtifactRef (its cases digested in
  order).
- `subject` and `baseline`: the admitted lock, the pinned door and
  judge door, and the run configuration.
- `runs`: per case, per arm, per attempt - the verdict of each grader
  with its explanation, plus the trajectory reference, the extension
  trace, and the stand-in call list.
- `coverage`: planned, attempted, completed, refused, failed,
  cancelled, and unknown counts per arm.
- `measurements`: the score per case (the weighted grader mean) and
  the suite score; under `--baseline without`, the delta per case and
  the suite delta, both as `subject - baseline`.
- `verdict`: `pass`, `fail`, or `inconclusive` under the declared
  threshold and the report's stated limitations.

`--json [path]` writes the result document instead of the summary, to
stdout or to the named file. `--report <path>` writes the standalone
HTML view elsewhere; with `--mocks`, its details show the calls and
guard failures. `--output-dir` redirects the whole result directory.

A case's run errors turn into grader failures: `timeout`,
`max_turns_exceeded`, `cost_ceiling`, `aborted` (a stand-in guard),
`auth_failed`, and `env_var_rejected`. A run that ends early keeps its
reason on the case; a suite that could not finish writes
`partialReason` (`cost_ceiling`, `auth_failed`) on the result.

Exit codes: `0` when the suite met the threshold, `1` on a sub-threshold
score, a load failure (no suite, malformed case, a case with zero
effective weight), or an internal error, `2` when the run is partial,
`64` for invalid usage, and `130`/`143` on `SIGINT`/`SIGTERM`. `--json`
still writes the result document on a nonzero exit.

## Publishing and the Gym

`--publish` turns the result into a claim: the runner's world key
signs the report digest and the harness emits a kind `3189`
evaluation publication naming the suite, the subject, and the exact
report ArtifactRef. The Gym's boards and other trainers' `reproduce`
quests read it. On the phone this is **ADD MY RESULT TO THE GYM**; the
CLI flag is the same act.

- An unpublished result exists only under `evals/results/` and leaves
  no remote trace. `--no-publish` is accepted for scripts that already
  publish; it is the default under `--baseline without` pilots.
- A published result is a `gym-trial` XP award candidate: XP is
  credited when another trainer's `reproduce` check confirms it, never
  for the run itself.
- A check is the same command over the published suite: a `reproduce`
  quest pins the report's digest, and a matching verdict on the same
  cases confirms the claim.

## Iterate on an extension against its suite

The suite closes the loop: change the extension, rerun, compare. The
loop's rules:

1. Work on the development partition only; a held-out partition, if
   the suite declares one, is touched once, to confirm a change that
   already passed.
2. Change one thing per round: a program step, a skill's wording, a
   guest's packet. Record each round in the suite's status table
   (change, development score, delta, verdict, report path) -
   including rounds that scored worse.
3. The noise floor is the spread between repeats of the same code. A
   change inside the spread is not a signal; rerun before keeping or
   reverting on it.
4. The keep rule: a change stays only if it passes at least as many
   cases as the baseline and improves score, cost, or time beyond the
   repeat spread. A component the graders never see help is dropped
   from the default set and then from the package.
5. Judge outputs are debugging material - read the failures before
   editing, and fix the extension, not the grader, unless the grader
   itself is wrong.

`openagents ext eval` supports `--case`, `--tag`, `--runs`, and
`--baseline` for each round; the cost flags (`--max-cost-usd`, the
judgment-door spend) bound a round's spend. A round's score comes from
the result document's `measurements`, never from memory of a prior
run's summary.

## Authoring a suite

`openagents ext eval init` in a terminal runs the interview inside a
Coder session on your machine: it reads the extension's manifest,
programs, guests, skills, and declared entries, then walks the steps
below, one step per turn, each gated on your explicit approval. It
never edits the extension itself. `--bare` writes the template and
skips the interview; no terminal and no name writes the template and
says so; `-i`/`--interactive` on a template name runs the interview.

The interview's shape (the full prompt is under
[the interview prompt](#the-interview-prompt)):

0. **Gate.** The interviewer checks the directory is a real extension
   and nothing is pending; it stops on an error rather than a guess.
1. **Read the extension.** Manifest, component kinds, declared
   entries, and each component's own docs. Name what the extension is
   for and who runs it - in the component's own words where they
   exist - and end with the boundaries: what it does on its own and
   what it leaves to the operator.
2. **Define quality.** Before any case, answer: what does a good run
   look like, and what does failure look like? The graders fall out
   of that answer.
3. **Source inputs.** At least 4 should-fire cases and 1-2
   should-not-fire cases, from real shape: the host product's
   actual screens and flows, not invented text. A case named for a
   real task beats a generic one.
4. **Design graders before operations.** Every case carries at least
   one outcome grader; `operation_used` graders confirm the extension
   was reached. The judge's rubric is the `criteria` body of a
   `judge` grader - write it for a judge that sees the trace.
5. **Mock stand-ins.** For each declared entry or door the extension
   calls, decide whether to stand it in, and write the fixed bodies,
   `expect` guards, and `abort_when` conditions.
6. **Pilot.** Run `openagents ext eval . --runs 1 --baseline without
   --no-publish`, read every run's trace and grader output, fix the
   suite, and repeat until the pilot is clean. Then estimate the full
   suite's cost and confirm before the first scored run.

Floor invariants the interview keeps: at least one should-not-fire
case; at least one outcome grader per case; `runs` at least 3; the
baseline arm on while a component resolves; and graders that measure
outcomes, not only that a tool ran.

## The system prompts

These prompts are the ones the harness and interviewer carry. The
braces are substitution points.

### The interview prompt

The interviewer turn runs under this system prompt:

```text
# Extension-eval authoring interview

You are the eval-authoring interviewer for the extension at ${path}.

You are running inside an interactive Coder session on the operator's
machine. The extension under authorship is the working directory unless
the operator points at another path.

## The hard rules

- You may read the extension (manifest, programs, guests, skills,
  docs, declared entries). You may not edit it.
- At every gate - after you describe the extension's purpose, after the
  case list, after the grader designs, after the cost estimate - stop
  and wait for an explicit yes before the next step. If the answer is
  not an approval, fix and re-ask.
- One interview step per turn. Do not run ahead.
- The floor invariants: at least one should-not-fire case, at least
  one outcome grader per case, `runs` >= 3, the baseline arm on while
  a component resolves, outcome graders over mechanism-only graders.
- A grader describes an observable property of the run. If a property
  cannot be observed in the trajectory, the produced files, or the
  stand-in calls, it is not a grader.
- Never write a case whose prompt tells the agent which operation to
  call. A case describes a task; whether the extension's operation
  fires is what the eval measures.
- When you finish, write the cases under the eval directory the
  operator chose, and close with the command that runs the suite.

## Steps

0. Confirm the target directory is an extension (a package manifest,
   or a directory of components the operator names). On any error,
   stop and report it.
1. Read the extension. State its purpose, the components it ships,
   and the entries it declares - in its own words where they exist -
   and what it explicitly does not do.
2. Ask the operator what good looks like: the outcome a run should
   reach when the extension works, and the ways it can fail.
3. Propose the case list: at least 4 should-fire and 1-2
   should-not-fire, each named for a real task shape. Wait for
   approval.
4. For each approved case, propose the graders. Every case has an
   outcome grader; add mechanism graders where they carry signal.
   Wait for approval.
5. For each declared entry or door, propose a stand-in or a reason to
   keep the real one. Wait for approval.
6. Pilot the suite with the cheapest run and read every result. Fix
   and repeat until clean. Then quote the full suite's cost and get
   approval before the scored run.
7. Write the suite. Report what was written and the command to run it.
```

### The judge prompts

The `judge` grader's system prompt:

```text
You are a strict, terse evaluation judge for agent runs.
```

Its user prompt:

```text
## Run record
${trace_or_output}

## Criterion
${criteria}

Answer only with "PASS" or "FAIL".
```

The judge answers three times; the grader passes when a strict
majority is `PASS` with no `FAIL`. The trace shown is the first and
last 12 steps plus the final message; file focus is the file's
contents.

The `baseline` grader's user prompt:

```text
## Baseline run record
${baseline_trace}

## New run record
${trace}

## Criterion
${criteria}

Does the new run satisfy the criterion at least as well as the
baseline? Answer only with "PASS" or "FAIL".
```

The `decision` grader sends the focus as `state` with the grader's
typed `question` to `POST /v1/systemone` and scores the returned
probability against `threshold`, three calls, majority.

### The stand-in responder prompt

When a stand-in is `type = "agent"`:

```text
You are standing in for the host entry "${entry}" inside an automated
evaluation run. The agent under test is calling the "${operation}"
operation.

${stand_in_instructions}

Respond with what the real entry would return for this call. Output
the response value only.

## The call
${input_json}

## Calls so far this run
${call_history}
```

When `abort_when` is set, the prompt additionally carries the
conditions and the instruction to answer with the abort token
`__EVAL_ABORT__:<reason>` only when one holds, and the run aborts with
the condition as the reason. The responder may not invent aborts
outside `abort_when`; the harness treats any other abort token as a
result.

### The in-session wrapper

`openagents ext eval init` inside an existing session prepends one
line to the interview prompt:

```text
This session is an eval-authoring interview. Use eval-authoring mode:
the interview prompt below is your instructions. The transcript the
operator sees is the interview's, not a coding session's.
```

## Availability

`ext eval` is on by default. The tenancy registry carries a capability
flag the CLI checks at startup with a bounded fetch (3 seconds,
fail-open: an unreachable registry leaves the command available); when
the flag is off the command prints `ext eval is currently unavailable`
and exits 1, so an operator kill switch works without a client
release.

## Reference: limits and constants

| Bound | Value |
| --- | --- |
| `runs` | 1-50, default 3 |
| `max_turns` | <= 200, default 10 |
| `deadline_seconds` | <= 3600, default 300 |
| `--concurrency` | 1-8, default 1 |
| Grader files per case | 256 |
| Case file size | 1 MiB |
| File-focus read | 10 MiB |
| Mock directory files | 256 |
| Mock file size | 1 MiB |
| Agent stand-in calls | 4 x `max_turns` per run |
| Judge votes | 3, strict majority |
| Capability fetch | 3 s, fail-open |

## What this page does not claim

- A passing suite does not admit an extension to a host; adoption is a
  separate decision under the package trust ladder.
- A stand-in proves nothing about the real entry; `--mocks off` or a
  live-door run is the check against the real thing.
- A `receipt` grader pass proves the same bytes behaved the same on
  this engine; it is not remote attestation and says nothing about
  correctness.
- The interview's approval gates are operator decisions, not
  automation; piping yes into the prompt forfeits them.
