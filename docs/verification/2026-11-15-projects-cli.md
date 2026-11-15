# Projects CLI implementation and live test drive

## Audit and changes

The installed `openagents` binary was the 2026-11-04 bundle. Its device login
called retired auth v2 routes. Current source already had the supported device
approval flow and shared account signing transport. Its singular `project`
domain operated the public Nostr work queue; it had no access to the new private
Projects API. That API also required a session ID, rejecting verified linked
NIP-98 keys.

The plural `projects` domain now uses the supported login/profile store, exact
body NIP-98 signing and the API's codecs. It exposes create/list/show/update,
instruction reads/writes, text knowledge, scanned file references, knowledge
list/content/removal, chat list/link/new/send/reopen/complete and admitted context
inspection. Replies preserve complete API JSON. An explicit revision fences
every write. Mutations print their operation ID before HTTP; retries use the
same command, file bytes and operation ID. New/send derive a stable completion
ID so both stages replay.

The API admits a verified person session or linked CLI key. Unlinked signers
remain refused. Additive chat-write v1alpha1 routes atomically save/link a new
thread, append a user turn after an assistant reply, and store the operation
receipt. Scoped reads join both account and Project. Existing frozen transcript,
metering and context code are reused. The old queue's internal helper names were
made distinct so the CLI can load the shared private-project codec without
collisions. Public queue command semantics stay the same.

The CLI build can emit portable C remotely and link it against the local SDK.
The existing built-CLI test process harness is shared by the script and Projects
checks. No extra secret store, transport, Markdown parser, model client or object
store was introduced.

## Verification

Focused checks pass: Project chat codec 133, saved chat codec 173, CLI account
rules 13 and URI rules 10 checks. Changed-procedure mutation gates killed all
82, 76, 15 and 52 mutants respectively; the Project access policy killed all
7 changed-procedure mutants and passed its 8 checks. No survivor or equivalent
was accepted. The descriptor oracle, pinned protobuf module/compatibility
checks, tools-check, wire-check and layer-check pass.

The shared CLI metadata/knowledge writers additionally pass all 133 codec
checks and kill all 29 changed-procedure mutants. Their checks enumerate
optional metadata fields, text/file sources and explicit selection, including
Unicode round trips and refusal of incomplete output.

Portable CLI C was compiled on a build host and linked on the Mac against its
SDK. Both fresh profiles authenticate through that binary. Staging device
approval linked `projects-staging` to its person account. The owner approved
`projects-owner` in the browser; `whoami` resolves the email-linked account,
distinct from the pre-existing admin key's account. Existing member Project
indexes can be read through the CLI. Private runtime responses stay in 0600
files under `~/.openagents/cli/project-test-drive/2026-11-15/`, not this repository.

The remote disposable API run passed 129 HTTP integration checks (71 history,
29 private Projects and 29 actual CLI checks), 21 route response goldens and
33,286 NIP-98 differential cases without disagreement. The CLI checks include
12,000-byte instructions, selected/unselected knowledge, exact create/send
replay with two model calls for two turns, scoped reopening, context inspection,
revision conflicts, removal and another account's read/write denials. Command:
`API_TESTS_ONLY=projects_checks.coax,projects_cli_checks.coax,history_checks.coax coax/web/api/test.sh`
through `coax/remote/remote.sh`; the target checks finished in 431 seconds.
The invocation's existing CLI authentication, director, World and append-only
log checks also passed; the complete run finished in 506 seconds.

The first attempt exposed main's missing stdbool.h include in the new station
adapter; main's fix `9aa8f8f906` was incorporated before the passing run. The
stale NIP-98 generated-cache warning is informational: the API builds its Coax
sources afresh. Concurrent router/station-report changes are retained.

The installed command now points to a new versioned CLI bundle, with its
existing adjacent helper executables preserved by pinned symlinks. The former
2026-11-04 bundle remains available for rollback.

## Deployment and live account test

Implementation commit `c8a1041545` was pushed to main. The ordinary API deploy
applied migrations and verified exclusive traffic, the routed health build and
the same immutable image on staging and production:

- Staging: `oa-api-staging-00230-wfc`.
- Production: `oa-api-00168-qk2`.
- Image digest: `sha256:71afdd1d54870049a5cb89788de1139bd5362654f18eecbc76ba489d4fddcc10`.

An independent stdlib JSON/SHA-256 oracle drove the installed CLI against the
real services. Staging passed 41 checks; the approved browser account passed
45. Each run created a private Project, changed metadata and instructions,
added selected and unselected knowledge, read the complete source text, started
a real model chat, reopened it, continued it, inspected its pinned context,
removed a source, saved a second chat without dispatching, completed it
explicitly, linked/replayed existing operations and verified final counts.

Real answers followed the saved instructions and used facts from the selected
source. The admitted context's instruction hash matched the independent SHA-256;
unselected knowledge was absent, and removing it preserved the earlier snapshot.
The final owned Project has one selected source and two saved conversations,
including a continued conversation. Whole-command replay after later changes
kept the original conversation and revision. A separate valid production
account received 404 for the owner's Project and conversation, and its index
excluded the owner's Project.

Account IDs, Project IDs, source bytes, messages and full live responses remain
in the private local receipt directory. They are deliberately absent from this
public verification record. No real existing Project was modified. The mobile
client continues to consume the same Project, history and saved-thread codecs;
this turn's live test used the CLI, with native UI checks retained from the
earlier Projects implementation.

The current consumer covers the operations actually implemented by the private
Projects API. Sharing, workspace tenancy, arbitrary document extraction,
knowledge replacement, archive/delete and external resource connections remain
unimplemented capabilities of the broader plan, not hidden CLI-only features.
