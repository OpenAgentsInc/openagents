# The first environment runtime

This implementation is the first shared-user slice of
[the environment plan](environments-claude-and-world-editing.md). Its defining
schema is [runtime.proto](../../proto/openagents/environment/access/v1alpha1/runtime.proto).
It uses the existing Claude SDK, engine, transcript reader, session log and
NIP-HOST signature/encryption/admission code. It does not claim full ENV
readiness, SESS artifact compliance, terminal takeover or realm previews.

`apps/shared/environment-app.coax` is the application on iPhone, macOS and
Linux. UIKit/AppKit execute its compiler-generated module in JavaScriptCore;
Linux executes the same Coax as C and projects its component tree with GTK.
None uses a WebView. Markdown is the component system's existing parser.
`coder host serve` owns Claude processes independently of these windows.

## Admission and carriage

The gateway's account-owned connection table maps an account and host public
key to one locally approved broker device/grant, epoch and relay. The catalog
never returns routing secrets or grants. GET reads require the account;
POST commands retain the API's human and limited-login restrictions.
The broker key is a Secret Manager file mount, not an environment value.
The host checks the current grant and epoch on every remote operation.

The explicit local `environment-enable` operation binds that one grant to a
canonical execution workspace and separately approved read-only history
root. It adds review authority for this optional profile only. A browser's
legacy realm grant alone does not enable it. Stop remote sharing by revoking
the grant with `forget`; local operator access remains independent.

The typed `app.rpc` methods are `DescribeEnvironment`, `ListSessions`,
`ReadSession`, `ReadSubagent`, `OpenSession`, `SendMessage`, `StopTurn`,
`StopSubagent` and `RespondInteraction`. `ReadSubagent` is a read: it names the
`item_id` of the call that started a sub-agent and pages that agent's own calls
and words (newest first, 24 at most); a computer older than it refuses the
request. `StopSubagent` is a write with the same right as `StopTurn`: it names
that `item_id` too (required there) and stops that one sub-agent of a Coder
Desktop chat's running turn (#13776). A computer older than it refuses the
method, and a phone older than it never sends it. They use the frozen HOST signed/encrypted kind3188
carriage and correlated replies. The optional projection lives above HOST
in Coder's `environment/host-wire.coax`; the old browser decoder stays closed.
There is no generic process or shell RPC.

The separate [execution preparation profile](environment-execution-v1alpha1.md)
adds `PrepareTask`, `PutTaskInput` and `ReadTaskInput` under the same explicit
environment/device/grant binding. They retain immutable input and upload
receipts across resident restarts. They do not start a task or widen this
Claude-only runtime contract.

`DescribeStation` is a separate read-only method for the delegation router
(docs/router/README.md). Its params are `openagents.router.describe.v1alpha1`
and its reply `openagents.router.station.v1alpha1`
(`proto/openagents/router/v1alpha1/router.proto`): the computer's OS, cores,
disk in the enrolled workspace, memory, load, the coding agents installed and
logged in, the agents running, and the repository it holds. It needs an
observe right on the live grant and the same enrolled device and grant as the
methods above, and reads no token, process argument or file content
(`coder/tui/host-station.coax`). The gateway carries it at
`GET /api/chat/environment/station/v1?host=KEY`, with a wait of eight seconds.

Local access uses the same request/view codecs, one JSON record followed by
LF per Unix connection. `service.sock` is0600 inside the host's0700 folder.
Both ends verify the actual kernel peer UID. The service singleton uses the
existing filesystem lock. The local adapter never treats localhost or a
caller-supplied PID as authorization. A configured desktop can use **This
computer** without an account or gateway connection.

## Commands and continuity

The client creates random128-bit session/command IDs once. Every operation
has a five-minute logical deadline with60 seconds of clock skew. The service
durably writes exact normalized intent before a control effect. An identical
retry returns its receipt without executing again; a changed body under the
same ID refuses. Transport loss means uncertainty, not permission to mint a
second command. A command already recorded remains queryable after expiry.

Managed writes check session ID, generation and expected revision. A second
approval cannot resolve an already answered request. Pending permissions
remain in the existing SDK until explicitly answered or stopped. Native
Terminal transcripts are inert and read only; the app starts a separately
owned managed conversation to control Claude. It never silently switches to
hosted inference. Provider credentials stay on the chosen computer.

Every view from an updated computer carries its Claude offer: the models a
new conversation can start on (from the engine's catalog, with `default`
for the CLI's own choice) and the effort levels each one takes. The phone and
desktop clients show a **Model** and an **Effort** picker above the composer
from that offer, and `OpenSession` and `SendMessage` name the chosen
`model` and `effort`. The computer starts Claude Code with `--model` and
`--effort`. On a running conversation it switches the model in place, and a
new effort restarts the CLI with that message, resuming the session. The
computer refuses a model or effort it does not offer with
`model_unavailable`. Each managed session summary names its current model
and effort. Writers leave both fields out when nothing is chosen. A computer
that sends no offer gets requests without them, and its clients show no
pickers.

The service retains four managed process slots. A managed chat holds one while
its Claude process runs and gives it back when that process ends, after ten
minutes with no turn, or when a new chat needs a slot and it is the least
recently used idle chat. A running turn, a waiting permission or a message
still on its way to the CLI keeps the slot. A chat given back while idle keeps
its Claude session ID beside its saved files (`environment-GRANT-ID.resume`),
reads as `available`, and continues on its next message, which takes a slot
again and resumes the session with `--resume`. A chat whose process ended
reads as observe-only. Capacity refuses explicitly, and only when all four
slots have a turn running or a permission waiting.
Closing/quit detaches the client. A service restart recovers saved histories
as observe-only/unknown instead of guessing that an old native writer can
be resumed safely. Full persisted process reconciliation and qualified native
continuation remain implementation work from the larger plan.

## Coder Desktop chats (2026-11-19, #13713)

Coder Desktop runs each chat's worker, and each of its sub-agents' workers, as
a daemon run (`StartRun` on the local socket). `ListSessions` on the first
page lists the running and finished top-level runs whose folder is the grant's
workspace or inside it, as `desktop-RUN` sessions with `"surface":
"coder-desktop"` (an optional field older clients ignore). The daemon reads
them from the run files only (`coder/tui/host-desktop.coax`): the chat's
first task and queue key from `RUN.in`, and its transcript from the worker's
lines in `RUN.out`. Each turn's words, thinking, commands, reads and
delegations become items; a delegation's call links to the sub-agent run whose
session is the chat's plus `-agent-`, which `ReadSubagent` pages. A finished
chat reads as observe-only.

On a running chat, `SendMessage` while a turn runs is a steer (the worker's
`steer` line); while it is idle it is a follow-up: a queue item that carries
the chat's last task with the new words, promoted at once, so Coder Desktop's
queue shows it. `StopTurn` interrupts the turn and stops its sub-agents.
`StopSubagent` stops one sub-agent alone: the daemon finds the sub-agent run the
item's call links to and writes the worker's stop line into its input, as Coder
Desktop's own one-agent Stop stops its worker; its siblings and the turn go on,
and Coder Desktop reads the worker's end, so the chat's run is told it stopped.
It is refused `not_found` for an item that is no sub-agent call, `conflict` when
that sub-agent has already ended or no turn runs, and `unsupported_profile` on
any session that is not a Coder Desktop chat. All three check the run's
generation and use the same idempotent command record as managed chats. The worker's accepted steer line carries its words, so Coder
Desktop shows a steer it did not send. `StartRun`, `StopRun`, `ListRuns` and
`ForgetRun` stay local-socket only.

## Bounds and failure

Owned timeline pages contain at most6000 raw text bytes, with UTF-8-safe
backward chunks for long messages and enough room for worst-case JSON
escaping. A managed cursor is `(part+1)*1000000 + byte_end`; zero requests
the current tail. A foreign/out-of-range cursor refuses. Disk recovery and
the live view share this projection. Tools show their input while pending
and their output when resolved. Session recording exhaustion stops execution.

Observed histories retain the shared reader's32MiB source and32MiB discovery
budgets,128-summary cap, bounded excerpts and explicit gap/truncation warnings.
The list pages16 rows; discovery incompleteness is distinct from no chats.
These are bounded views, not a claim of lossless native history. Saved managed
catalogs currently have an eight-row recovery bound. The full source-epoch,
artifact and history-cut semantics in the larger plan remain required before
advertising full SESS history support.

## Installation and checks

A phone pairs with this computer with no OpenAgents account:

```sh
openagents-environment pair --workspace ~/code/project --label "Mac"
```

It prints an `openagents://pair?host=…&relay=…&code=…&name=…` link. Open it on
the phone (OpenAgents asks this computer to pair); the computer allows only the
request whose label ends with that code, for 7 days, and writes its own
enrollment for that workspace. Revoke it with `coder host forget ID`. The
phone's side is `apps/shared/app/device/` and `apps/shared/native/AppDeviceLink.swift`.

The native targets are `apps/openagents-desktop/native/build.sh` (Mac) and
`native/build-linux.sh` (Linux SDK link after generating `program.coax`,
`linux-services.coax` and `coder/tui/host.coax`). The Linux desktop identity
remains `com.openagents.desktop`; app files live under `~/.openagents/`.
Mac account credentials retain their existing file/device-key adapter.
Linux uses the existing Coax login flow and its registered token record in
`~/.openagents/desktop/native-login.json`, without overwriting legacy files.
The first Linux email profile has no native device proof key; a bound login
is refused rather than forging proof.

The owner deployments use a launchd/user-systemd service named
`com.openagents.coax.environment`, with the binary behind
`~/.openagents/bin/openagents-environment`. The Linux legacy host key is
retained. The older installed Mac host remains a recovery service with its
Keychain identity; this new enrollment does not export or rotate its key.
The old desktop shell remains a recovery option until migration gates pass.
GUI first-run enrollment, general workspace configuration and Windows are
still separate qualification work; the owner's two machines are configured
through the same local CLI boundary in this slice.

On a Mac with Coder Desktop, one binary is the resident daemon (GitHub #13757).
`openagents-environment` and Coder's bundled `coder-host` are the same program,
`coder/tui/host.coax`. `apps/coder-desktop/install.sh` copies the installed
Coder's `coder-host` to `~/.openagents/environment/bin/COMMIT/` and points
`~/.openagents/bin/openagents-environment` at it. It also sets the login service
to run that link as `serve --standby`. A standby that finds another daemon
holding `service.lock` waits and takes over when that daemon stops; it doesn't
exit for launchd to restart it every 10 seconds. A plain `serve` still exits
with status 4. The script restarts the daemon after an install, and on each
15-minute check, only while the daemon has no worker running. Otherwise it waits
for a later check. Coder starts its own `coder-host` only when no daemon answers.

Checks include authority mutation, exact-byte golden records, real filesystem
durability, the API's delivered-email/account isolation tests, generated
Wasm/JavaScript frame parity, native UIKit reducer/composer tests and actual
Claude replies on both computers. See the release verification record for
the exact deployed build and remaining owner checks.

### Compiled client provider Usage adapter (2026-11-16)

The protected local Unix transport additionally accepts the already published
`openagents.environment.accounts.request.v1` profile and returns its existing
`accounts.view.v1`. Runtime v1alpha1 and HOST envelopes keep their identities
and shapes. Kernel peer authentication, explicit provider enablement and exact
configured device/grant binding precede dispatch. Provider requests and replies
use `coax/lib/agent/accounts/wire.coax`; `local-request.coax` selects between
those existing profiles, without a new envelope or second codec.

The compiled GUI uses an internal FX_PROVIDER boundary with private-free request
metadata. An authorization-code marker is replaced exclusively inside the SDK,
never transmitted. The exact existing request is retained in protected storage
before a mutation. Reopening exposes only its public metadata for an explicit
same-command retry; a correlated but unknown receipt cannot release ownership.
This internal ABI is not a change to any published Runtime or provider profile.

## The agent a phone's new chat runs on (2026-11-20)

A phone's `OpenSession` runs on an agent this computer can run, never on an
engine whose CLI is missing (`coax/lib/agent/environment/phone-agent-policy.coax`,
`coder/tui/host-phone.coax`). The offer lists only the engines the computer
runs, its working default first: Microcoder's Gemini models when `mcworker
--auth` finds a sign-in, `antigravity` when the Antigravity CLI is installed,
then Claude Code's catalog when `claude` is installed. The request's `model`
picks its engine when the computer runs it; otherwise the chat runs on the
default in that order. With none of them the computer refuses `no_agent`.

Claude Code chats keep the managed slot above. A Microcoder or Antigravity
chat is a daemon run the host starts itself, whose run id is the phone's
session id: the phone keeps the session it opened (no `desktop-` prefix), and
it is read and written as a Coder Desktop chat is, with `"surface":
"coder-desktop"`. `RUN.phone` keeps its generation and engine. A follow-up is
a task line; after its worker ended the run starts again under the same id,
told the conversation so far (the worker's `recall` line), so the chat stays
`available`.

A notice item may say its kind in `notice_kind` (`install`, `login`,
`limit`), and readers draw it as a setup notice about the computer, never as
the agent's reply. Older computers send none; readers still draw a `notice`
item as a notice.
