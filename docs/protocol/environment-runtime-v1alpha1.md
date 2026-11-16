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
`ReadSession`, `OpenSession`, `SendMessage`, `StopTurn` and
`RespondInteraction`. They use the frozen HOST signed/encrypted kind3188
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

Checks include authority mutation, exact-byte golden records, real filesystem
durability, the API's delivered-email/account isolation tests, generated
Wasm/JavaScript frame parity, native UIKit reducer/composer tests and actual
Claude replies on both computers. See the release verification record for
the exact deployed build and remaining owner checks.
