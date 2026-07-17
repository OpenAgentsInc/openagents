# Command Code Teardown — 2026-07-16

## TL;DR

Command Code is the first coding-agent product in this teardown set whose main
claim is not a new model, editor, or orchestration topology. Its differentiated
product is a **learned preference compiler**. The closed `command-code` npm
package observes corrections, can import Claude Code, Cursor, and Codex
sessions, can mine Git history for wrong-to-right change patterns, and writes
portable Markdown `taste.md` packages with numeric confidence. The normal file
tools are explicitly prevented from modifying those packages; a separate
learning agent owns them. Project, personal, and remote scopes make the result
shareable without pretending that an opaque model checkpoint is portable.

That is a valuable product seam for OpenAgents. Preferences should not remain
an informal mixture of `AGENTS.md`, transcript recall, and presentation
settings. OpenAgents should eventually add a governed preference-learning
pipeline with typed observations, reviewable candidate rules, evidence refs,
confidence and freshness, explicit activation, and reversible owner
disposition. A learned preference may influence planning or code review. It
must never widen tools, filesystem, execution, spend, publication, or release
authority.

The implementation also shows why OpenAgents should not copy the product
wholesale:

- the published CLI is closed and `UNLICENSED`; the public repositories are
  README, brand, issue, and upstream-VS-Code reference surfaces rather than the
  engine source;
- the 1.2 MB minified JavaScript bundle still reveals a broad Node/React/Ink
  runtime, a hosted generation gateway, local JSONL sessions, checkpoints,
  MCP, skills, hooks, custom agents, telemetry, and a self-updater;
- session persistence atomically rewrites the entire current transcript with
  regenerated record ids instead of durably appending accepted events;
- default/plan/auto-accept permissions and hook policy are useful
  authorization UX, but the package contains no evidence of a local OS
  sandbox; `--yolo` deliberately bypasses prompts;
- documentation calls Taste learning local, while the bundle sends prompt
  batches and compiled learning context to the Command Code generation API;
- an authenticated background fingerprint hashes machine id, MAC addresses,
  OS username, hostname, and Git email and also reports hardware and timezone,
  a materially broader disclosure surface than the telemetry page explains;
- the bundled VS Code extension has unusually good bounds and filesystem
  modes, but trusts any process running as the same OS user that can discover
  its Unix socket; and
- the unauthenticated CLI path exposed small release-quality seams: `cmd info`
  reported an unknown version, `cmd status` recommended a command spelling
  different from top-level help, and the advertised `cmd taste learn` command
  is absent from the Taste subcommand registry.

The central recommendation is therefore: **adapt Taste as a governed,
evidence-bearing preference plane; retain OpenAgents' typed authority,
containment, durable admission, receipts, signed updates, and open
load-bearing seams.**

## 1. Snapshot identity, provenance, and confidence

### 1.1 Audited artifacts

| Artifact | Identity | What it proves |
| --- | --- | --- |
| Public `CommandCodeAI/command-code` repository | `a774fe8cbe71697d115d4660de299c9c1b286cea`, 2026-06-26 | Product README, install command, links, issue forms, and brand assets; no engine source |
| Archived `CommandCodeAI/cmd-old-public` | `48cacf798aa213f88cd5d2be12187a91c793bddf`, 2026-03-10 | The same public product README and brand shell; no historical engine source |
| `CommandCodeAI/vscode` fork | `e6ab937aa21a1d1d71646b1d802e29e03de62610`, 2026-07-05 | A current Microsoft VS Code mirror plus an unmerged Command Code terminal-title detection commit; not the Command Code engine |
| npm package | `command-code@0.51.0`, published 2026-07-16 | Closed production CLI bundle, package manifest, bundled skills, and VS Code extension |
| CLI bundle digest | `e18fcf04feca14fc349736088dbbbb2873334bae110a83b42ca70293ecf8a001` SHA-256 | Exact `dist/cli.mjs` audited here |
| VSIX digest | `ec490d06a9d647fa9248d42976474c10c74c839299c2eb20b3403c42a97197ed` SHA-256 | Exact bundled `commandcode-vscode.vsix` audited here |
| Public docs and changelog | read 2026-07-16 | Intended UX, privacy, permissions, Taste, sessions, extension, and release claims |
| Isolated runtime smoke | Node 24.13.1, macOS arm64, temporary `HOME` and config root | Version/help/info/status behavior without touching a real Command Code account or user data |

The npm registry reported integrity
`sha512-Hq/0aw5XvyKZn+aU+pNTBASRUlaIwhsd6irvNR8BN67wj/SFm2tRRuE4tgoXEq2ef6yVVrAh1dvvi9ktnu7qOQ==`.
The install added 360 packages in the isolated audit prefix. [bundle]

### 1.2 Evidence labels

This document uses the directory convention:

- **`[bundle]`** — package manifest, compiled JavaScript, VSIX, source map, or
  bundled skill;
- **`[runtime]`** — observed behavior from the isolated command invocation;
- **`[source]`** — commit-pinned public repository evidence;
- **`[public]`** — current official documentation, changelog, registry, or
  product page;
- **`[inferred]`** — a conclusion supported by several observations but not a
  vendor claim; and
- **`[limitation]`** — something the available evidence cannot establish.

### 1.3 Important limitations

The package is minified but not opaque: function names, schemas, prompts,
routes, state paths, policy logic, and the complete VS Code extension source
map remain visible. That is enough to assess client architecture. It cannot
prove how the hosted API stores prompts, implements `taste-1`, enforces account
policy, routes providers, or deletes server-side data. No real login, model
request, Taste upload, MCP OAuth, shared session, auto-update, or cloud sandbox
was performed. Private credentials and real project/session contents were not
read. [limitation]

## 2. Public-source reality: the product is closed

The active and archived product repositories contain only `readme.md`, issue
templates, and brand assets. Their histories document README and issue-form
changes, not the CLI implementation. The npm manifest explicitly declares the
package `UNLICENSED`; only its small VS Code extension declares Apache-2.0.
[source] [bundle]

The organization also hosts a `vscode` fork. Its checked-out `main` is a current
Microsoft upstream mirror, not a Command Code IDE product. A separate commit by
the founder adds Command Code to VS Code's terminal agent-title recognition:
the shell type, Windows executable detection, OSC-title pattern, setting copy,
and tests. That commit is not an ancestor of the audited fork's `main`.
[source]

The honest product description is therefore:

```text
public product and integration shell
  README + docs + issue tracker + brand assets + upstream VS Code proposal

closed shipped client
  npm launcher
    -> minified Node/React/Ink CLI
    -> bundled skills
    -> small source-mapped VS Code/Cursor/Windsurf extension
    -> Command Code generation/auth/billing/Taste services
```

## 3. Product thesis: correction is the scarce asset

Most coding agents compete on model access, tool breadth, editor integration,
or parallelism. Command Code's launch story instead starts with repeated
correction: TypeScript rather than JavaScript, a particular bundler, a
particular test runner, a package manager, naming choices, and architectural
micro-decisions. Rules files help, but the company argues that manually written
rules decay while repeated corrections can compound into a current profile.
[public]

That is a strong framing. The product treats four things as different:

| Plane | Command Code representation | Purpose |
| --- | --- | --- |
| Explicit instructions | project/user `AGENTS.md` | Rules the owner or team intentionally wrote |
| Learned preference | project/global `taste.md` packages | Inferred coding choices with confidence |
| Product settings | project/local/user JSON | Enablement, presentation, hooks, permissions, providers |
| Conversation state | per-project JSONL and metadata | Recover and continue a particular session |

Many agents collapse all four into “memory.” Command Code's separation is its
most important architectural contribution. [bundle] [public]

## 4. Packaging and runtime shape

### 4.1 npm launcher and process model

Four binaries — `cmd`, `cmdc`, `command-code`, and `commandcode` — point to the
same tiny ESM launcher, which sets `NODE_ENV=production` and imports the 1.2 MB
`cli.mjs`. The client is a Node process using React 19 and Ink 6.6 for the TUI.
Its dependency graph includes Vercel AI SDK provider packages, Zod, Commander,
OpenTelemetry, shell parsing, glob/minimatch/ignore, Jimp, diffing, Markdown
rendering, GitHub download support, and MCP/client utilities. [bundle]

This is a conventional, portable terminal application rather than a native
binary or an editor fork. The shipped VSIX is only about 48 KB; the CLI remains
the engine and product shell. [bundle]

### 4.2 CLI surface

The audited help exposes:

- interactive, initial-prompt, print/headless, plan, resume, continue, fork,
  and trust modes;
- standard, plan, auto-accept, and bypass permission postures;
- model selection and per-task model configuration;
- project directory grants;
- Taste learning from a repository and imports from other coding agents;
- memory, skills, custom agents, MCP, hooks, checkpoints, PR review, GitHub PR
  comments, sharing, usage, feedback, tracing, updates, and IDE setup; and
- autonomous `/goal`, background shell monitoring, compaction, context
  inspection, and session-file diagnostics. [bundle] [runtime]

The UX is broad for a single terminal process. The closest reference is Claude
Code's “terminal as application platform,” but Command Code is smaller,
JavaScript-based, and organized around a hosted generation API plus the Taste
loop rather than a public bidirectional engine protocol.

### 4.3 Hosted generation seam

The main generation path constructs config, memory, Taste, skills, permission
mode, model messages, tools, reasoning effort, and token limits, then posts a
streaming request to `https://api.commandcode.ai/alpha/generate`. It attaches
project, session, provider, Taste-learning, version, and optional OAuth headers
and consumes newline-delimited streaming events for text, reasoning, tool
calls, tool results, usage, finish, abort, and error. [bundle]

The bundle also contains endpoints for models, web search/fetch, sharing,
billing, usage, lifecycle events, Taste packages, profiles, organizations,
API keys, and an experimental sandbox service. Those strings prove client
capability, not server behavior or general availability. [bundle]

## 5. Taste: the differentiated system

### 5.1 Three input paths

Command Code can learn preference from at least three sources:

1. **Live interaction.** The learning agent watches new user messages and
   assistant/tool outcomes, processes pending messages serially, and updates
   project Taste packages. The UI can show a learning feed. [bundle]
2. **Other coding-agent sessions.** The import path scans Claude Code, Cursor,
   and Codex session stores for the current project. Cursor discovery uses
   agent-transcript JSONL directories; Codex discovery scans session JSONL and
   matches the recorded working directory. [bundle]
3. **Repository history.** `cmd taste learn <source>` accepts a local Git
   repository or GitHub repository, mines commit diffs and substitutions, and
   builds a prompt emphasizing wrong-to-right corrections, recurring themes,
   anti-patterns, and actionable opinionated rules rather than generic best
   practices. The bundle gates this subcommand behind its experimental-mode
   predicate even though default help and public docs advertise it, so the
   ordinary audited runtime did not expose it. [bundle] [public] [runtime]

This is materially better than generic semantic retrieval over arbitrary old
messages. Each source is trying to answer “what repeated decision changed the
owner's desired code?” rather than “what text is similar?”

### 5.2 Storage and portability

Taste packages are Markdown:

```text
project: .commandcode/taste/<category>/taste.md
global:  ~/.commandcode/taste/<category>/taste.md
remote:  commandcode.ai/<namespace>/taste
```

Each learning has the form `- <preference>. Confidence: <0..1>`. Files are
linted for categories, confidence syntax/range, and empty or malformed
entries. Push/pull can merge new, upgraded, downgraded, or unchanged learnings;
packages may be personal, organizational, or public. [bundle] [public]

Human-readable artifacts are the right portability primitive. They can be
reviewed, diffed, committed when project-scoped, removed, and imported without
requiring the original model weights. This is more useful than claiming that a
hidden embedding or vendor profile is portable.

### 5.3 A separate writer boundary

The ordinary read/write/edit tools reject paths inside project or global Taste
directories with a specific message that those files are managed by the
learning system. The learning agent receives a narrowed tool set, validates
that paths remain inside the Taste root, and only admits `taste.md` at the root
or a category directory. Path traversal is rejected. [bundle]

This is the best implementation choice in the product. Learned preferences are
not simply self-edited prose inside the same authority plane as coding work.
There is a named compiler with a restricted output grammar and destination.

### 5.4 Enablement and scope

Taste learning resolves local project, shared project, and user settings; the
local project override wins. It is on when unset. The `/taste` UI can change
project or user state. Project-shared enablement lives in
`.commandcode/settings.json`, personal override in
`.commandcode/settings.local.json`, and the user default in
`~/.commandcode/config.json`. [bundle] [public]

Default-on maximizes the product's compounding loop, but it is the wrong default
for a system that may inspect private sessions and send material to a hosted
derivation model. A visible feed is not equivalent to informed consent.

### 5.5 “Local” is underspecified

The security page says Taste learning runs locally and stores structured rules,
not snippets. The bundle does store the resulting Markdown locally. It also:

- batches raw prompts into an observer prompt and sends them to
  `/alpha/generate` for short learning-feed observations; and
- sends compiled repository correction context to a learning agent backed by
  the configured generation service. [bundle] [public]

This does not prove the service stores or trains on that material. It does mean
“local learning” cannot honestly be read as “all derivation happens on the
device.” The product needs separate disclosures for local artifact custody,
remote inference processing, optional cloud sync, and model training.

### 5.6 Missing evidence semantics

The artifact records preference text and a numeric confidence. The audited
format does not require:

- observation/session/commit refs;
- which accepted, rejected, edited, reverted, reviewed, or merged action
  supplied the signal;
- the derivation model and prompt/compiler version;
- first/last observed timestamps or recency decay;
- contradictory observations and conflict resolution;
- project, language, subsystem, path, task, or team applicability predicates;
- owner review state;
- activation count and downstream success/failure; or
- a reversible link from a bad application back to the contributing rule.

Confidence without evidence and calibration is a persuasive number, not a
governed fact. OpenAgents should take the compiler boundary and reject the thin
evidence model.

## 6. Instructions, memory, context, and skills

### 6.1 Explicit memory is `AGENTS.md`

Command Code calls `AGENTS.md` memory. It loads enterprise, user, project, and
nested directory instructions, supports imports, and displays their source
paths in the assembled memory context. Project files can live at the root or
under `.commandcode`; personal instructions live under
`~/.commandcode/AGENTS.md`. [bundle] [public]

The naming is loose, but the scope order and source labeling are useful.
OpenAgents should continue using a typed context envelope so explicit
instructions, learned preferences, selected artifacts, skills, and retrieved
history remain distinguishable at runtime.

### 6.2 Skills use progressive disclosure

Skills are Markdown packages discovered from Command Code and `.agents`
locations at user, working-directory, and Git-root scopes. Startup loads
summary metadata; activation loads the full `SKILL.md` and referenced files.
The CLI can discover and atomically install GitHub-hosted skills into a
temporary directory before rename, validates lowercase safe names, and rejects
local-path installation through that command. [bundle] [public]

Two skills are bundled: browser operation and an extensive design skill. The
design skill's command family includes review, smell, deslop, typography,
color, motion, interaction, responsive, redesign, tokenization, setup, finish,
refine, voice, and surface passes. [bundle]

This validates the same progressive-disclosure and provenance requirements
already present in the OpenAgents extension analysis. It does not establish
isolation: skills are instruction and potential authority requests, not an OS
sandbox.

## 7. Conversation engine and persistence

### 7.1 Message and stream model

The local schema distinguishes user, assistant, tool, bash, system, error,
info, Taste onboarding, command result, IDE status, and reasoning feed entries.
The hosted stream returns typed text/reasoning/tool/result/finish/error events.
The UI can reconstruct a feed and summarize tool inputs, but local feed types
and provider messages remain intertwined in one large process. [bundle]

### 7.2 Session storage

Sessions live beneath:

```text
~/.commandcode/projects/<slug>/
  <session>.jsonl
  <session>.meta.json
  <session>.share.json
  <session>.checkpoints.jsonl
  <session>.prompts.jsonl
```

The manager serializes save calls, hashes the message collection to skip
duplicates, writes a process-specific temporary file, and renames it over the
session JSONL. It tolerates corrupt lines on load and migrates a legacy message
shape. [bundle]

The important limitation is that `saveMessages` rewrites the complete current
message collection and generates fresh record and parent ids for every write.
This is atomic-file persistence, not an append-only accepted-event log. A crash
before the next save can lose accepted live state; regenerated ids are not
stable event identities; and a replay cannot distinguish durable admission,
flush, projection, and volatile display. [bundle] [inferred]

### 7.3 Resume, fork, and headless separation

Resume and continue restore the saved session model. Interactive and headless
history are separated by entrypoint so automation does not clutter the normal
picker, while an explicit id can bring a headless run into interactive mode.
Fork copies the transcript, checkpoint log, file-history directory, and model,
then writes parent session, fork time, branch point, and optional title into
new metadata. Partial fork failure cleans up created artifacts. [bundle]

That is strong local-product behavior. OpenAgents should keep the UX while
retaining stable Thread/Turn/Item/Work Unit refs and durable admission rather
than copying filesystem snapshots as canonical identity.

## 8. Checkpoints and rewind

Command Code creates a checkpoint per user message and backs up original file
content before edits. The rewind UI can restore conversation only, code only,
or both. It verifies backups before restore and documents all-or-nothing
behavior, a 10 MB per-file backup ceiling, disk-full pause, binary-file support,
and per-session scope. [public] [bundle]

This is better than a vague “undo agent changes” button. Three independent
restore modes correctly acknowledge that conversational and filesystem state
can diverge. The remaining gaps are the same ones identified in the Claude,
Codex, T3, and Grok analyses:

- restore does not undo shell, network, MCP, database, GitHub, or publication
  side effects;
- copied file history is not a Git baseline or durable worktree resource;
- a size-excluded file weakens the snapshot boundary; and
- conversation truncation plus code restoration still needs an explicit
  irreversible-effect disclosure and receipt.

OpenAgents should adapt the user model through staged inspect/restore and
conflict-aware worktree checkpoints, not treat a content backup as universal
rollback.

## 9. Tools, shell tasks, agents, MCP, and hooks

### 9.1 Built-in tool family

The compiled registry includes single/multiple file reads, edit/write,
directory listing, grep, glob, foreground shell, monitored background shell,
monitor events, shell task listing, TODO writes, web search/fetch, user
questions, process/port kill, plan enter/exit, and the Explore custom-agent
tool. MCP tools and Markdown-defined custom agents are added dynamically.
[bundle]

Large reads are bounded and report partiality. Background commands have task
ids, persistent output files, bounded tail/delta reads, lifecycle cleanup, and
the ability to wake the assistant from monitored output. The changelog records
multiple fixes for output bounds, hangs, orphaned MCP children, Windows
processes, and long-session memory. [bundle] [public]

### 9.2 Custom agents

Built-in Explore and Plan agents and user/project Markdown agents have separate
system prompts, tool selections, and context windows. Agent definitions are
loaded from project and personal directories and become callable tools.
[bundle] [public]

The client clearly supports delegation. The available artifact does not prove
a complete durable child graph, independent child transcript navigation,
mailbox semantics, recursive non-amplification, delivery acknowledgement, or
review/acceptance state. OpenAgents' existing child-topology and return-path
contracts remain stronger. [limitation]

### 9.3 MCP

MCP configurations merge user, per-project-local, and project `.mcp.json`
scopes. The CLI manages stdio and HTTP servers, OAuth token storage, discovery,
connection state, tool schema conversion, execution, and shutdown. Token
storage is local under `.commandcode`. [bundle] [public]

The product exposes transport and configuration plumbing directly. OpenAgents
should retain MCP compatibility but present capability, account, provenance,
policy, health, update, and removal as the primary lifecycle.

### 9.4 Hooks

Pre-tool hooks can allow or deny; post-tool hooks are advisory; stop hooks can
force up to three revisions; session-start hooks can inject context. Hooks are
shell commands receiving JSON over stdin. Project hook trust is fingerprinted
and retained separately, while user hooks are trusted by scope. [bundle]
[public]

This is a useful deterministic policy seam. It is not containment. A trusted
hook is arbitrary host code, and a model-facing injected string is context,
not authority. OpenAgents should compile hook-like extensions into named,
versioned capability generations with failure policy and receipts.

## 10. Permission and security posture

### 10.1 Useful defaults

Interactive default allows reads and asks for writes/shell. Plan blocks
mutation. Auto-accept allows it. Headless mode denies edits, writes, and shell
by default and exposes stable exit codes for authentication, permission, rate,
network, server, and interrupt failures. Project trust is prompted on first
use, and `--add-dir` explicitly widens filesystem context. [public] [bundle]

The read-only headless default is an excellent choice and should remain a
conformance requirement for every OpenAgents automation adapter.

### 10.2 Authorization is not containment

The package contains a permission service, allow/deny patterns, trusted paths,
trusted shell commands, compound-command parsing, and explicit `--yolo` /
`--dangerously-skip-permissions`. It does not ship a Seatbelt, Landlock,
Windows sandbox, VM, container, or equivalent local containment runtime.
[bundle]

An experimental remote sandbox command and API route exist in the bundle, but
that does not contain ordinary local interactive or headless execution.
[bundle] [limitation]

The correct assessment is:

```text
project trust + permission mode + pattern policy + hooks
  = authorization and review UX

OS sandbox / guest / capability broker
  = not established for ordinary local execution
```

### 10.3 Taste cannot grant authority

The hosted request includes Taste beside memory, skills, tools, and permission
mode. That separation is good. There is no reason a learned preference should
ever alter the latter. OpenAgents should encode this as an invariant: a
preference can rank or condition behavior only inside already-admitted
authority. It cannot select a broader execution target, grant a tool, suppress
an approval, publish, spend, or convert an agent result into acceptance.

## 11. IDE integration

The bundled extension supports VS Code, Cursor, and Windsurf. It publishes a
small session file under `~/.commandcode/ide`, listens on a Unix socket or
Windows named pipe, and answers two requests: current editor context and
diagnostics. Context contains workspace, active file metadata, cursor,
selection capped at 10,000 characters, and visible file metadata. The CLI
filters returned context against allowed workspace directories. [bundle]

The extension's implementation is unusually disciplined for a companion
bridge:

- session directory mode `0700`, socket/session-file mode `0600`;
- atomic session-file publication;
- 16-connection cap;
- 60-second idle timeout;
- 8 MB accumulated-buffer and 4 MB message caps;
- newline-framed request/response with only two action names; and
- cleanup on deactivate. [bundle]

The VSIX also ships a complete source map with `sourcesContent`, making this
part inspectable despite the closed CLI. [bundle]

There is no per-request token or challenge. Any process with the same OS-user
authority that discovers the socket can request editor selection and
diagnostics. Filesystem modes make that a deliberate same-user trust boundary,
not a remotely exposed unauthenticated service. OpenAgents should copy the
bounds and narrow vocabulary while retaining its host-owned typed capability
and generation fence.

## 12. Telemetry, fingerprinting, and privacy

The telemetry page says anonymous session, command-name, error-category, and
performance data are collected; it denies file paths, arguments, user input,
code, project structure, credentials, and personally identifiable information.
It documents a 30-day retention period and opt-out through config or
`DO_NOT_TRACK`. [public]

The client has a bounded OpenTelemetry queue, batch size, delay, export timeout,
content-capture flag, redaction helpers, and Axiom trace endpoint. That is a
more deliberate implementation than raw analytics calls scattered through the
UI. [bundle]

Separately, after authentication, a background fingerprint collector gathers:

- machine id;
- MAC addresses;
- OS username;
- hostname;
- global Git email;
- platform, architecture, OS version;
- CPU model/count, memory size;
- container state; and
- timezone.

It hashes the first five categories with a product salt, derives a stable
thumbmark, and posts the components to a fingerprint endpoint. [bundle]

Hashed identifiers remain persistent pseudonymous identifiers. The docs do not
explain this collection, its purpose, retention, relationship to telemetry
opt-out, or deletion. It may be abuse prevention or account security rather
than telemetry, but that makes a separate disclosure more necessary, not less.
[inferred]

The isolated `cmd info` command also printed host and OS username. That may be
appropriate for a local support report, but the UI should warn before users
paste it publicly. [runtime]

## 13. Updates and supply chain

The client has a non-blocking stable-release checker, lock, staged pending
update, restart/reload path, update status file, fallback/manual hints, and
failure reporting. The changelog records earlier periods where the auto-updater
was broken and users had to reinstall manually, followed by staged-swap and
stable-only fixes. [bundle] [public]

The package does not expose a signed component manifest, artifact signature,
compatibility ledger, retained rollback slot, or installation receipt. npm
integrity protects registry transport/content identity but is not the same as
publisher-signed release authority. The CLI, VSIX, skills, model registry,
hosted gateway, and docs can move independently without one user-visible
compatibility record. [bundle] [limitation]

OpenAgents should retain Command Code's background staging ergonomics but keep
its existing signed-manifest, immutable-candidate, fail-closed verification,
rollback, and receipt requirements.

## 14. Runtime and release-quality observations

The isolated smoke did not authenticate or enter a real project session.
Within that boundary:

- `--version` returned `0.51.0`;
- `--help` rendered the full command, slash-command, shortcut, and examples
  surface;
- `info` reported `Version Unknown` despite the working version flag;
- `status` correctly reported unauthenticated but recommended `cmd auth login`,
  while top-level help advertises `cmd login`; and
- top-level help and examples advertise `cmd taste learn <source>`, but the
  bundle adds that command only when experimental mode is enabled;
  `cmd taste --help` lists only push, pull, list, lint, and open, while
  `cmd taste learn --help` falls back to that same parent help with exit 0.
  The separate `cmd learn-taste --help` path does exist for importing previous
  agent sessions. [runtime]

These are not core-architecture failures. They demonstrate why CLI help,
non-TTY behavior, auth gating, diagnostics, and every documented command path
need release oracles rather than spot checks.

## 15. Comparison with the reference set

| Concern | Command Code | Stronger reference | OpenAgents consequence |
| --- | --- | --- | --- |
| Learned coding preference | First-class Taste compiler and packages | None in this set | Adapt with typed evidence and owner governance |
| Explicit instructions | Layered `AGENTS.md` | Claude Code provenance hierarchy | Keep source/scope/hash in one context envelope |
| Session persistence | Atomic whole-file JSONL rewrite | Codex SQLite + JSONL, OpenCode V2 admission/replay | Do not weaken stable ids or durable admission |
| Fork/rewind UX | Strong session fork and three-mode rewind | Claude/Codex/Grok worktree lifecycle | Adapt UX above guarded baseline-aware resources |
| Terminal host | React/Ink, background tasks, monitors | Grok Build | Keep Grok as renderer/process-lifecycle reference |
| Multi-provider | Broad hosted model catalog | T3 Code harness boundary | Model choice is not a substitute for open peer/runtime contracts |
| IDE bridge | Tiny bounded same-user socket adapter | OpenCode thin client/server split | Adapt caps and visibility under typed host authority |
| Permissions | Ask/plan/auto/bypass + hooks | Codex compiled sandbox policy | Keep authorization and containment separate |
| Extensions | Skills, agents, MCP, hooks | Claude/OpenCode/Grok breadth | Resolve into signed isolated generations |
| Portable sessions | Same-machine JSONL and cloud share | OpenAgents pathway target | Retain host-neutral identity, authority, and receipts |
| Updates | Staged npm self-update | T3/OpenCode build matrix; OpenAgents fail-closed ledger | Take staging UX, not unsigned authority |

## 16. What OpenAgents should adapt

### 16.1 Build a governed preference plane

The minimum honest lifecycle is:

```text
explicitly admitted observation
  -> typed candidate preference
  -> evidence + scope + confidence + freshness
  -> owner review or bounded auto-activation policy
  -> active preference generation
  -> application event with “why”
  -> outcome / correction / contradiction
  -> reinforce, narrow, suspend, supersede, or delete
```

The core records should be distinct:

| Record | Required facts |
| --- | --- |
| Observation | source kind/ref, owner/scope, timestamp, privacy class, accepted/rejected/edited/reverted/reviewed outcome |
| Candidate | normalized claim, proposed applicability predicate, compiler/model/version, contributing observation refs |
| Preference | stable id, text/typed constraint, scope, confidence, freshness, conflicts, status, generation |
| Activation | exact preference generation admitted into an exact turn/work unit and why |
| Outcome | applied/ignored/contradicted, resulting correction, verification/acceptance refs |
| Disposition | approve, narrow, suspend, supersede, delete, share, or export with owner and receipt |

Markdown can be an import/export and review format. It should not be the only
canonical state if rules drive automated behavior across devices.

### 16.2 Keep three memory classes visible

OpenAgents should name and display:

1. **instructions** — owner/team-authored normative context;
2. **learned preferences** — inferred, fallible, evidence-bearing behavioral
   guidance; and
3. **retrieved history/knowledge** — source facts selected for the current
   task.

Presentation preferences remain a fourth, non-model plane. A user should be
able to answer “who wrote this, why is it here, when was it last supported, and
what can it affect?” for every context item.

### 16.3 Add explicit importers, not ambient surveillance

Repository-history and foreign-session import are useful onboarding paths.
They must be explicit jobs with preview, source count, bounded date/repository
scope, private processing policy, cancellation, deletion, and an import
receipt. They must not scan all provider histories by default.

The selector for relevance/applicability must follow the workspace invariant:
typed semantic selection, embedding similarity, or a modeled structured
planner — not ad hoc user-facing keyword routing.

### 16.4 Put preference application in the workroom

The workroom should expose a compact “why this choice” affordance when a learned
preference materially changes a plan, implementation, review, or tool proposal.
Users should be able to correct once and choose whether that correction is
turn-only, repository-scoped, personal, or a reviewed team candidate.

Do not turn the learning feed into ambient celebratory noise. It should show
candidate, evidence, scope, confidence/freshness, and the action available.

### 16.5 Preserve Command Code's smaller good patterns

Adapt these independently of Taste:

- read-only-by-default headless execution with stable exit categories;
- model preserved per resumed session without silently rewriting the global
  default;
- explicit headless-history separation with opt-in promotion to interactive;
- fork provenance and partial-failure cleanup;
- separate code/conversation/both rewind choices;
- bounded same-user IDE context bridge with visible selection disclosure;
- progressive skill disclosure;
- monitored background command output with ids and bounded delta reads; and
- staged, non-blocking update UX beneath OpenAgents' stronger signed authority.

## 17. What OpenAgents should reject

1. **No default-on private-history learning.** Observation admission must be
   visible and controllable by source and scope.
2. **No confidence without evidence.** A decimal alone is not provenance,
   calibration, freshness, or review.
3. **No preference-authority widening.** Learned text cannot grant tools,
   filesystem, shell, network, account, spend, publication, acceptance, or
   settlement authority.
4. **No hosted/local ambiguity.** Local storage, local derivation, remote
   inference, cloud sync, and training use are separate disclosures.
5. **No fingerprinting outside the privacy contract.** Persistent device or
   account-abuse signals need typed purpose, minimization, retention, opt-out
   or necessity basis, and deletion semantics.
6. **No whole-transcript rewrite as canonical durability.** Keep stable refs,
   append-only admission/evidence, replay, projection, and repair.
7. **No permission UI presented as containment.** Ordinary host execution needs
   an explicit effective isolation statement and receipt.
8. **No self-updater as release authority.** Signed manifests, compatibility,
   rollback, and receipts remain mandatory.
9. **No TUI as the OpenAgents product shell.** Command Code is a strong CLI;
   OpenAgents' product remains Desktop/mobile with terminal as one bounded
   foreign host.
10. **No closed load-bearing preference engine.** The schema, compiler
    boundary, applicability rules, and evidence semantics should be public and
    testable even when a selected inference model is external.

## 18. Recommended sequence

1. Add a dated product/assurance design for governed preference learning;
   define observation, candidate, preference, activation, outcome, and
   disposition before adding UI.
2. Prove a local-only repository correction importer against synthetic Git
   histories; no foreign provider session scan and no remote model dependency
   in the first proof.
3. Add owner review, conflict/freshness handling, deletion, and Markdown
   import/export.
4. Integrate active preference generations into the existing typed context
   envelope with a hard authority non-amplification test.
5. Add workroom “why applied” and one-turn/repository/personal correction
   controls.
6. Add explicit Claude/Codex/Cursor session import only after privacy preview,
   bounded source disclosure, and delete receipts exist.
7. Add optional team sharing through owner/scope policy and signed generation
   identity; do not infer team norms from one person's corrections.
8. Evaluate remote derivation only as an explicit processor with disclosed
   content class, provider, retention, and receipt.

This is a post-core-loop pathway, not permission to interrupt the current
Desktop reliability and portable-session gates.

## Final assessment

Command Code's breadth is credible, but breadth is not the main lesson. Its
important idea is that the correction loop can become a product and an
artifact: learned coding preference can be separate from explicit instructions,
portable across projects, visible to the user, and maintained by a restricted
compiler rather than by the ordinary coding agent.

OpenAgents should take that idea further. A preference plane should be typed,
evidence-bearing, scoped, fresh, conflict-aware, reviewable, reversible, and
incapable of widening authority. With those properties it becomes more than a
rules file and safer than an opaque personalization model. Without them,
“continuous learning” is simply another ambient context source whose mistakes
compound as quickly as its successes.

## Primary source map

### Commit-pinned local references

- `projects/commandcodeai/repos/command-code/readme.md`
- `projects/commandcodeai/repos/cmd-old-public/readme.md`
- `projects/commandcodeai/repos/vscode` at
  `e6ab937aa21a1d1d71646b1d802e29e03de62610`
- Command Code terminal-recognition proposal commit
  `f7148cd9685c3fb4838aba6cc4dafdedf84de067`

### Official public references

- <https://commandcode.ai/docs>
- <https://commandcode.ai/docs/taste>
- <https://commandcode.ai/docs/core-concepts/interactive-mode>
- <https://commandcode.ai/docs/core-concepts/headless>
- <https://commandcode.ai/docs/core-concepts/custom-agents>
- <https://commandcode.ai/docs/core-concepts/checkpoints>
- <https://commandcode.ai/docs/core-concepts/memory>
- <https://commandcode.ai/docs/core-concepts/ide-integration>
- <https://commandcode.ai/docs/skills>
- <https://commandcode.ai/docs/mcp>
- <https://commandcode.ai/docs/hooks>
- <https://commandcode.ai/docs/troubleshooting/telemetry>
- <https://commandcode.ai/docs/resources/security>
- <https://commandcode.ai/launch>
- <https://commandcode.ai/changelog>
- <https://www.npmjs.com/package/command-code>
