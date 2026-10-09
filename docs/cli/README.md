# The `openagents` command

`openagents` is one program that reaches every OpenAgents surface an agent
needs, over Nostr: pairing with the owner's computers, ordering and steering
Coder work, standing in the Verse, driving the Lagrange construction zone,
and drafting sovereign-agent profiles under NIP-SOV. It replaces the forge
API client of the same name that `dabc08102` removed; nothing in it talks to
the forge.

The crate is `crates/openagents-cli`. It adds no protocol logic of its own:
`host`, `pair`, and `task` are the `coder` implementations; `computer` is
`coder_computers::live::Live`, the same client the Computers screens use;
`verse` and `zone` are `verse::net`, `verse::mv`, and `verse-lagrange`;
`sov` validates the NIP-SOV profile shape and refuses to activate anything
until the pieces NIP-SOV requires exist.

## Conventions

- `--json` anywhere on the command line before `--` switches every command to one JSON
  document (or one document per line for streaming commands) on stdout.
  Without it, output is a short table or sentence.
- Exit codes: `0` success, `1` refused or failed, `64` invalid usage.
- `openagents doctor` prints the identities, stores, and relays the command
  uses and whether each exists. `openagents COMMAND --help` prints each
  group's syntax.
- Keys live under `~/.openagents/`: Verse profile keys in `verse/`, the
  Computers device key in `coder-computers/`, host and access records in
  `host/` and `coder-access/`, and `openagents chat`'s own device key in
  `chat/`. No command prints a secret key, a bearer
  key, or an invitation after the moment it is created.

## Build and install

Install a release (macOS and Linux; Windows and channels in
[the terminal guide](../terminal/README.md#install)):

```sh
curl -fsSL https://storage.googleapis.com/openagentsgemini-cli-releases/openagents/install.sh | sh
```

Or build it from a checkout:

```sh
cargo build --release -p openagents-cli
install target/release/openagents ~/.local/bin/
```

## OpenAgents Terminal (`openagents terminal`)

`openagents terminal`, or bare `openagents` on a terminal, opens OpenAgents
Terminal: a full-screen chat with OpenAgents over the same client as
`openagents chat`, where a coding reply runs Coder on this computer and its
steps stream into the screen. Esc stops a reply or a run, Ctrl+T lists
threads, and `/help` lists the slash commands. It opens on a new thread;
`--continue` (the last thread used in this folder), `--thread ID`,
`--scratch`, `--local`, and `--socket PATH` choose otherwise. Bare `openagents` piped or with `--json`
still prints the usage and exits 64. The guide is
[docs/terminal](../terminal/README.md).

## Chat with OpenAgents (`openagents chat`)

`openagents chat "How do I connect a phone"` sends a message to OpenAgents,
the chat router the phone and the desktop use, and streams the reply. The
unit is the thread: `--thread ID` continues one, `chat threads` lists them,
`chat read --thread ID` prints one, and `chat export --thread ID` prints its
ATIF trajectory. When this computer's host runs, threads are the desktop
app's; otherwise the command keeps its own under `~/.openagents/chat/`, and
`--scratch` uses a throwaway identity. `--json` streams NDJSON events with
the router's typed metadata, and `--run-coder` accepts a Coder offer through
the host. The full guide is [chat.md](chat.md).

## Models on the OpenAgents API (`openagents inference`)

A plain HTTP client of the public inference API
(`docs/inference/gateway.md`; OpenAPI at `BASE/openapi.json`).

```sh
openagents inference google/gemini-3.8-flash "Say hello."     # the answer's words
openagents inference gemini-3.8-flash --input "Hi"           # a bare name is the catalog's id
openagents inference openagents/chat - --stream < prompt.txt # stdin, printed as it arrives
openagents inference openagents/fast --json '{"input":"Hi","temperature":0.2}' --format json
openagents inference openagents/code "Fix it" --api chat --format events
openagents inference openagents/chat "Hi" --max-price 0.50/2.00 --max-output-tokens 200
openagents inference models
openagents inference rates
openagents inference google/gemini-3.8-flash "Hi" --pay x402 --max-msat 10000   # no key
```

- `--base URL` (or `OPENAGENTS_API_BASE`) points it at another gateway,
  such as a local one (`http://127.0.0.1:PORT/v1`); the default is
  `https://api.openagents.com/v1`.
- `--key` (or `OPENAGENTS_API_KEY`) is an `oak_` key; requests draw on its
  account's credit.
- `--json BODY` after `inference` is the request body, not the global
  output switch; `openagents --json inference ...` still sets JSON output.
- `--pay x402` (no key): the `402`'s invoice is checked against the
  request and the spending policy, paid from the wallet (`--pay-with
  wallet|node`, `--max-fee-msat`), and the same bytes are sent again with
  `PAYMENT-SIGNATURE`, as `openagents x402 fetch` does.

## Private sales records (`openagents sales`)

`openagents sales` reads and updates the host's private lead/account pipeline
through `coder::task::sales`. Every operation requires an explicit `--root`;
initialization writes a private owner credential, and subsequent operations
require that human's `--credential FILE`. Credentials and exports must remain
outside the store's `sales` directory. Use `init`, `issue`, `revoke`, `apply`,
`list`, `show`, `export`, `audit`, and `suppressed`; `--help` describes their
syntax. The [sales pipeline guide](../sales/README.md#private-sales-pipeline)
explains conditional revisions, exact-byte retries, recipient boundaries,
accepted handoffs, retention, and suppression. The command contacts no
prospect and starts no agent. `sales claims source`, `review`, `read`, `draft`,
`validate`, `withdraw`, and `history` expose the same private store's
[reviewed claims register](../sales/evidence.md#reviewed-claims-and-prices).
Draft reads recheck current source, price, playbook, and comparative evidence;
proposed prices remain unavailable without separately recorded activation.

The owner can enroll separately consented funnel journeys through `apply`, read
their original scope with `show`/`export --journey JOURNEY`, and run `weekly` over
explicit private exports, accepted task evidence, and checked finances. `review`
rechecks current custody and writes owner-approved delayed counts with no
publication. The [weekly evidence guide](../sales/evidence.md#consented-weekly-operating-review)
defines the manifests, review, privacy boundaries, and qualification limits.

The owner configures a versioned mailbox with `sales email apply --input FILE`
and reads its evidence with `email view`. `email check --input FILE
--mailbox-key FILE` prepares an exact, permissioned US business email; it returns
only digests and expiry and sends nothing. The private provider credential keeps
its actual SMTP, OAuth, or API format: 1 to 2,048 UTF-8 bytes without a newline or
NUL. A host can instead inject a sealed credential source, encrypted with an
existing host account authority key. Neither path creates a keychain item or
gives an agent the credential. Preparation checks current suppression, lead and
policy revisions, provider recipients, template, sender identity, footer,
unsubscribe evidence, and credential custody again after a blocking lookup.

`email evidence --input FILE --message-sha256 SHA` maps bounded provider evidence
without treating acceptance as delivery. A missing observation remains unknown.
Domain, sender, TLS, authentication, and unsubscribe evidence is owner-declared;
fixture checks advertise no live sender. The durable outbox separately owns any
dispatch authority. Complete the [outreach owner checks](../../NEEDS_OWNER.md#sales-outreach-launch)
before real use.

`sales outbox propose` freezes one exact owner approval subject, including the
recipient, MIME bytes, attachment digests, current authority, and count reservation.
A selected SMTP endpoint also requires its exact
`provider:email:CONFIG_ID:HOSTNAME` recipient in the lead and policy grants.
`outbox apply` approves that digest or records an attributable owner send, rejection,
pause, reconciliation evidence, or owner-reviewed restart.
`outbox view` returns one typed subject for supported renderers and identifies
phone or lectern surfaces whose Sales owner binding is unavailable. `outbox dispatch` consumes the approval once through implicit TLS SMTP;
it releases the sales lock during network waits and reopens current authority
before credentials, recipients, and body disclosure. Native certification, original
cost attribution, reply handling, consent, suppression, and owner activation must
all be available. Live agent messages pin an exact graded native draft and its
original helper expense, use the subject `Requested business information`, and
refuse attachments until an explicit graded attachment contract exists.
An uncertain attempt consumes its reservation and cannot replay.
`outbox fixture` uses isolated evidence, counts separately, and cannot qualify a
live operating week or raise the outreach cap. SMTP acceptance does not prove delivery.
Owner reconciliation preserves the original uncertain attempt and consumed count;
it never asserts delivery or enables a second send. Attachment filenames and
proposal metadata receive the same credential and customer screening as content.
The transport follows [SMTP reply semantics](https://www.rfc-editor.org/info/rfc5321/)
and [implicit TLS submission](https://www.rfc-editor.org/info/rfc8314/).

`sales replies ingest` accepts private, bounded inbox imports with explicit untrusted
provenance and exact original thread references. Contact safety runs before owner
classification: opt-outs suppress contact, and hard bounces or material safety
findings pause outbound work. Quotes, links, and attachment metadata grant no tools
or permissions. `replies review` records one exact owner classification and returns
its native artifact for the outbox's `reply_reference`;
`replies qualify` retains measured native scratch-injection results, and
`replies revoke` removes the current qualification and pauses the channel.
Automatic mailbox polling, independent inbound provider delivery receipts, and model
quality qualification remain unavailable. Owner imports cannot prove delivery or
managed deletion of the original import file. No-response `replies follow-up` plans
use original native acceptance timestamps, at least 168 hours between attempts,
and at most two contact-wide follow-ups. Salted contact identity survives message
minimization and changes to leads or agents. A plan still requires current permission,
certification, suppression, and exact owner approval before dispatch.

## Commercial customer (`openagents customer`)

`openagents customer` binds the installed client to the existing gateway account,
workspace, payer, and decision resource. It requires an absolute private `--root`
and explicit credential aliases; it never uses a provider login, paired host,
wallet, or ambient API key as spending authority. Import a key with `import
--alias NAME --input FILE`, read its account with `account --origin URL --alias
NAME`, then use `select --origin URL --alias NAME --account ID --workspace ID
--door NAME`. `current` reads fresh rights or reports an unavailable connection;
`history` and `show --purchase ID` retain the selected customer's original records.

`commercial --product gateway|plugin` reads the operator-reviewed binding for
the selected native account and workspace. The returned canonical customer can
differ from the native payer. It imports no other product records, balances, or
spending rights. Quotes retain the original binding revision; a changed mapping
requires renewed selection and approval. Plugin purchases separately pin their
Plugin source before approval and payment. An absent mapping establishes no
cross-product identity.

`quote --purchase ID --input FILE` freezes the exact private decision request,
payer, price, and charge ceiling. Review the emitted quote, approve its exact
`quote_digest` with `approve --purchase ID --digest DIGEST`, then use `invoke
--purchase ID` once. Rotation or changed rights require a new quote and approval.
An interrupted dispatch remains unknown and blocks another purchase for that
payer; `reconcile --purchase ID [--receipt DIGEST]` only reads original settlement
evidence. Invocation prints the requested result; retain it privately if needed.
Receipt reconciliation verifies the frozen artifact identity, including its
model, adapter, signature, and execution settings. A door alias can differ from
the checkpoint name in that receipt.

`change --input FILE` reads a private JSON intent with `id`, `origin`, `account`,
`credential_alias`, and `action`. Actions are `sign-in` with `output_alias`,
`rotate` with `workspace`, `key`, and `output_alias`, `revoke` with `workspace` and
`key`, or `sign-out`. `recover` uses `workspace`, `output_alias`,
`credential_alias: null`, and a separate private `--recovery-token FILE`.
Issued aliases are immutable; selecting one remains a separate command.
Exact retries read the retained outcome without repeating the mutation.
Unknown outcomes refuse renamed retries. `inspect --operation ID` authenticates
a retained once-issued credential after interruption without acknowledging an
unknown historical effect or silently switching accounts. `credentials` reads
operation references for the selected customer.

`team members --workspace ID` reads current roles, members, invitation metadata,
and the selected door's payer context. `team switch --workspace ID --door NAME
[--alias NAME]` refreshes admission before selecting a workspace for future
purchases. Existing quotes, approvals, holds, and receipts keep their original
account and payer.

`team change --input FILE` reads a private intent with `id`, `origin`, `account`,
`credential_alias`, and `action`. Actions are `create` with `name` and `seats`,
`invite` with `workspace`, `role`, and `ttl_secs`, `accept` with `workspace`,
`withdraw` with `workspace` and `invitation`, `role` with `workspace`, `account`,
and `role`, or `remove` and `transfer` with `workspace` and `account`. Invitation
and role changes support `admin` or `member`; ownership uses `transfer`.
An invitation returns a private `invitation_file` path. Transfer that file only
through an authorized private handoff, then accept it with `--invitation FILE`.
The CLI sends no invitation and prints no token. Acceptance pins the reviewed
workspace and file's displayed role against the canonical token before
membership changes. Tokens expire and can be accepted once.
Gateways without reviewed acceptance refuse before a membership effect.
`team inspect --operation ID` reads a retained intent under current account and
workspace rights. Unknown effects block renamed retries; inspect current
invitations and membership before reviewing another action. Account recovery
does not restore revoked team membership or grant another member's keys.

`funding --input FILE` sends one private JSON intent to the selected gateway's
optional decision-funding route. Use `{"op":"quote","id":"funding-one",
"amount_msat":200000}` to review an exact BTC funding quote, then
`{"op":"issue","id":"funding-one","approved":"QUOTE_DIGEST"}` to approve
that retained quote. Invoice creation does not pay an invoice. The gateway
credits the original workspace only after its admitted receiver observes
confirmed collection. `read` and `reconcile` intents name the same immutable
`id`; they cannot substitute a customer, payer, or receiver. Mutations make one
HTTP attempt. After an interrupted response, inspect that original identity
before another invoice. `funding-history` retains verified observations and
uncertain intents even when current credentials are unavailable.

The selected funding lane uses BTC currency-millionths: one ledger unit is
100,000 millisatoshis (100 satoshis). Fractional units refuse. Funding, reserved
decision liability, observed settled usage, and wallet liquidity remain separate;
funding itself earns no usage revenue. The gateway must enable this adapter and
provision the customer's native monetary account and policy first.

Inputs require private regular files, and the store uses mode `0700` directories
and `0600` files. HTTPS origins are supported; HTTP is restricted to explicit
loopback fixtures. Missing account, funding, or resource admission remains
unavailable. Qualify the installed connection and selected commercial offer
through [the owner gates](../../NEEDS_OWNER.md#customer-connection-o8-rev-09-10816).

## Coder (`openagents coder`)

`openagents coder` exposes the Coder terminal's chat runtime, plugin settings,
model selection, and discovered ACP agents. Settings use the same private store
under `~/.openagents/coder-new`; `--state DIR` selects another store, and `--in DIR`
selects a working directory. Chat and delegation stream NDJSON with `--json`.

```sh
openagents coder status --json
openagents coder chat -p "Review the parser" --session parser-review --json
openagents coder models set openai/gpt-6-luna:low
openagents coder plugins enable microcoder
openagents coder agents list --json
openagents coder delegate microcoder --task "Add a parser regression test" --json
openagents coder sessions read parser-review --json
openagents coder export parser-review --output parser-review.atif.json
openagents coder import parser-review.atif.json --session imported-review
```

Repeat `chat --session ID` to continue a chat. Add `--delegation ID` to continue
one child while preserving the parent and other children. `chat --demo` runs
without a model request. `models list` includes the built-in OpenAgents gateway
and models supplied by enabled plugins.

`plugins configure openrouter-byok --stdin` and `plugins configure jev --stdin`
accept a JSON object with `api_key`, `model`, `endpoint` (Jev only), and `enabled`.
Use `api_key: null` to remove a saved key. Keys can also come from the terminal's
supported environment variables and `.env` file. Commands never print a saved
key. Run `openagents coder --help` for each command's syntax.

Retained chats are ATIF-v1.8 documents under the store's `sessions` directory.
Import and export run no tools. The terminal's `/export [path]` writes the selected
chat, including child trajectories when exporting the main chat, to a private
local file. Without a path, it writes under `~/.openagents/exports`.

## Leases (`openagents lease`)

`openagents lease` runs a command while it holds a lease from the host
resource broker, so agents on one machine share build slots, memory, disk,
the quiet machine, the screen, the GPU, and licensed tools instead of
fighting over them. [Leases](../coder/runtime/leases.md) covers the
resources, defaults, overrides, and receipts.

```sh
openagents lease build -- cargo test -p coder-lease
openagents lease quiet --receipt soak-lease.json -- scripts/grid-soak.sh
openagents lease memory --amount 24 --no-wait -- ./train.sh
openagents lease run --class release-gate -- ./scripts/verify-rust.sh --release
openagents lease list --json
openagents lease grant screen --for 30m
openagents lease revoke screen
```

`lease RESOURCE` waits its turn unless `--no-wait` is given, and exits with
the command's status. With `--json` it prints the receipt after the
command's output. `lease list` shows holders and waiters, and `lease grant
screen` asks the owner to confirm on an interactive terminal. `lease run
--class CLASS` places a release gate, benchmark, soak, or build here or on
another computer by the `coder.placement` setting;
[Placement](../coder/runtime/placement.md) covers it.

## Durable scratch (`openagents scratch`)

`openagents scratch` creates the calling session's private scratch directory
under `~/.openagents/scratch/` and prints its path, so an agent keeps
captures and scripts somewhere a reboot doesn't clear. `--session SESSION`
names another session, and `--json` prints the path, session, and root.
[Durable scratch](../coder/guides/scratch.md) covers the rules.

## Single-digest artifacts (`openagents artifact`)

`openagents artifact submit NAME` queues a branch's change to an artifact
in the `artifacts/` registry, such as `everglade-pack`, and runs the queue,
which lands changes one at a time on `origin/main` with one repin per
batch. `artifact queue` lists pending changes (`--all` adds landed and
rejected ones with their reasons), and `artifact run NAME` runs the queue.
[The artifact queue](../coder/runtime/artifact-queue.md) covers it.

## Browser checks (`openagents browser`)

`openagents browser run -- CMD` starts Chrome with a fresh profile and its own
debugging port, gives `CMD` `OPENAGENTS_CHROME_PORT` and
`OPENAGENTS_CHROME_WS`, and removes the profile when `CMD` ends, so agents can
verify in a browser at the same time. `--headed` takes the screen and browser
leases. [Verify in a browser](../coder/guides/browser.md) covers the details.

## Local capability settings (`openagents settings`)

`openagents settings show|get|set|unset` edits `~/.openagents/settings.json`,
which `openagents chat`, the desktop, and a host on this computer read: the
coding agents Coder may use and their order, whether a coding reply runs at
once or asks first, the usage threshold, the project folders, and what a
local run's commands may reach. See [settings.md](settings.md).

## Knowledge entries (NIP-KB)

Use `openagents kb` to work with local knowledge entries and NIP-KB relay
events. `--json` returns one JSON document on stdout. Success exits `0`,
refused or failed operations exit `1`, and invalid arguments exit `64`.
The network commands require an explicit relay and a positive timeout in
seconds. They use the knowledge signing key unless you pass `--key-file`.

```sh
openagents kb search "relay handoff" --lexical --limit 5 --json
openagents kb show entry-id --dir knowledge --json
openagents kb withdraw entry-id --reason "Outdated advice" --json
openagents kb publish entry-id --relay wss://relay.example --timeout 30 --json
openagents kb sync --relay wss://relay.example --timeout 30 --author NPUB --json
openagents kb head entry-id --relay wss://relay.example --timeout 30 --author NPUB --json
```

`search` returns ranked hits with scores, entry metadata, and the reason
when ranking uses only words. `show` returns the parsed entry, its full
document, and any pending version. `withdraw` updates the local entry and
returns its new status; use `publish` to send the withdrawal to a relay.
`publish` returns counts and per-entry outcomes for the selected IDs (or
every local entry when you omit IDs). `sync` verifies remote events and
returns accepted entries, withdrawals, refusals, and incomplete query
counts. `head` returns the author's current verified entry pointer or
fails if that pointer is missing, invalid, or withdrawn. Use `--author`
to read another author's head; without it, the command uses the signing
key's public identity. Run `openagents kb --help` for the full syntax.

## Pairing and computers (NIP-HOST, NIP-REACH)

Enroll this device with a host, list hosts, and order work. The host
authorizes every call against its own grant records; the command only
carries the request.

```sh
openagents host init --owner OWNER_PUBKEY --relay wss://relay.openagents.com \
  --workspace repo=$HOME/work/repo      # a host on this machine
openagents host serve
openagents host invite --rights standard # a coder-host: invitation for another device
openagents computer link 'coder-host:...' # redeem one here
openagents computer link --ssh user@box  # install and adopt a host over ssh
openagents computer list --json
openagents computer devices HOST
openagents computer invite HOST --rights standard --days 30
openagents computer approve HOST ENROLLMENT --code 1234
openagents computer revoke HOST DEVICE
openagents computer workspaces HOST
openagents computer task HOST --workspace repo --title "Fix CI" Make the flaky test deterministic
openagents computer steer HOST TASK --revision 2 Prefer the smaller change
openagents computer cancel HOST TASK --revision 3
```

### Shell commands on a linked host (NIP-TERM)

`exec` opens a shell on the host over its current route (loopback, tailnet,
direct, or relay; the Computers supervisor picks it), replaces the shell
with the command, and returns the command's output and exit code. The
command runs in the host's workspace with the host's environment, so a
`claude` or `codex` session the host is logged into is available to it.
`shell` is the same terminal, interactive. Both need the host to have
granted this device the `terminal` right.

```sh
openagents computer exec HOST -- git status --short
openagents computer exec HOST --timeout 1800 --json -- codex exec "add a test for the parser"
openagents computer shell HOST              # Ctrl-] detaches; the shell keeps running
```

`exec` exits with the command's code, `124` when `--timeout SECONDS`
(default 600) passed first, and `1` when the host refused. With `--json`
it prints `{"output", "exit", "timed_out", "shell", "route"}` instead of
streaming.

### Watching a host (aliases, watch, tail, journal)

Every command that names a `HOST` accepts an alias or a unique prefix of
the host key as well as the full key. Aliases live in `aliases.json` in
the computer store.

```sh
openagents computer alias coderos 93235bef…        # name a host
openagents computer alias --list
openagents computer watch coderos --every 30 --until "reward" -- tail -1 ~/runs/x.log
openagents computer tail coderos ~/runs/x.log --lines 20 --follow
openagents computer journal [HOST] [--lines N] [--json]
```

`watch` reruns a command every `--every` seconds until its output
contains `--until TEXT`, it exits 0 with `--until-exit`, or `--for
SECONDS` passes; with `--json` each iteration is one NDJSON record. `tail`
runs `tail -n N [-F]` on the host and, with `--json`, prints one
`{"path", "when", "line"}` record per line. Every `exec`, `watch`, and
`tail` appends one line to `exec.jsonl` in the store: when, host, the
command, exit code, seconds, output size, and a SHA-256 of the output, but
not the output itself. `journal` reads it back.

### Study runs on a host (`openagents study`)

`study` drives Microcoder study runs in a directory on a host: `run`
starts one detached through the directory's runner script, and the reads
parse the logs, `summary.json`, `outcomes.txt`, and `events.jsonl`
there.

```sh
openagents study run coderos ~/gates-reasoning-runs sound-change-cascade full 1 --runner ./one-claude.sh
openagents study status coderos ~/gates-reasoning-runs     # step and state per run
openagents study status coderos ~/gates-reasoning-runs --until-done 1500  # the host waits for the runs to end first
openagents study outcomes coderos ~/gates-reasoning-runs   # reward, steps, time, usd, ending
openagents study faults coderos ~/gates-reasoning-runs     # steps whose model call failed
```

Each goes through `computer exec`, so the journal records it too.

`openagents task` is the durable local queue (`coder task`), and
`openagents pair` shows the QR code that reads this computer's chats on a
phone.

### Paired chats (`openagents session`, NIP-SESS)

`session` is the other end of `openagents pair`: it redeems a `coder-pair:`
invitation with the `--as` profile key, keeps the public connection code
under `~/.openagents/session-observer/` (`OPENAGENTS_SESSION_HOME` or `--store PATH`
overrides it), and reads the retained Codex and Claude chats the computer
disclosed, through `coder-connect`'s history-observer client. The
`coder-connect` binary keeps working as before; it is the host side.

```sh
openagents session pair 'coder-pair:...' --timeout 20     # the invitation's relay; --relay must match
openagents session connections                             # saved grants, hosts, relays, sources
openagents session list --json                             # every chat each paired computer discloses
openagents session read CHAT --limit 50                    # records from the start of one chat
openagents session tail CHAT --timeout 120                 # records as the chat grows, one line each
openagents session steer CHAT "prefer the smaller change"  # refused: exit 1
openagents session interrupt CHAT                          # refused: exit 1
openagents session forget GRANT                            # drop the saved connection; the host still holds the grant
```

The command migrates an existing `~/.openagents/session/` observer directory
to the new path on first use. The older Coder Terminal GitHub sign-in flow
writes its login token to `~/.openagents/session`, as recorded in the
[terminal scope](../terminal/scope.md). That file stays untouched. Explicit store overrides are not migrated. If both
directories exist, the command asks you to consolidate them without overwriting
either store. With no saved connections, run `openagents pair` on the other
computer, then run `openagents session pair INVITATION` here.

`CHAT` is the chat ID that `list` prints, or its source ID. `read` pages
through the transcript until it has `--limit` records (default 200) or the
source ends; with `--json` it prints one document with the chat, the
records (raw bytes as `raw_base64`, the reader's `readable` projection
when there is one), `has_more`, and the next cursor. `tail` polls with the
retained cursor every two seconds until `--timeout` (default 30) passes and
prints one record per line. Every request has the observer's bounds: 32 KiB
of raw bytes per page, 240 reads per minute per grant.

`steer` and `interrupt` exist so an agent learns the answer from the grant
rather than from a missing command. The observer profile's
`openagents.history-observer-grant.v1` admits observation only, so both read
the saved grants and exit `1` with the reason; control of a task goes
through `openagents computer steer` and `openagents computer cancel` under
a NIP-HOST grant. The invitation is never printed or logged after `pair`
redeems it.

## Host service and SSH hosts

`service` runs the resident host under the service manager through
`coder-service`: a launchd agent on macOS or a systemd user unit on Linux.
The service runs the `coder-service` launcher, which starts the host, trials
each update, and restores its snapshot when a trial fails. Each command reads
the host root (`--root DIR`, default `~/.openagents/host`), and `--json`
prints the status, report, or descriptor that `coder-service` keeps there.

```sh
openagents service install --host-key HEX --launcher ~/bin/coder-service  # register and start
openagents service status        # service manager view, committed version, descriptor
openagents service update --to SHA256 --wait 120   # committed exits 0; rolled back or unfinished exits 1
openagents service descriptor    # host key, generation, state, and the latest update
openagents service restart
openagents service uninstall     # state, bundles, and logs stay
```

`install` needs a staged bundle (`--version SHA256`, or the one
`scripts/coder-host.py` selected) and the `coder-service` binary: `--launcher
PATH`, or `coder-service` in the directory that holds `openagents`. Arguments
after `--` replace the host's default `host serve`. See
[Host service](../coder/runtime/host-service.md) for the trial and rollback
phases.

`ssh` is the launcher side of the NIP-ENV SSH-launched host profile, through
`coder-ssh`. `add` installs the pinned `coder` release on an account you
reach with `ssh`, starts a host or adopts one that already runs, and redeems
its invitation on this device, so it appears in `openagents computer list`.

```sh
openagents ssh add me@box --owner OWNER_PUBKEY --archive linux/x86_64=coder-linux-x86_64.tar.gz \
  --relay wss://relay.example/ --timeout 300
openagents ssh tunnel me@box --for 3600   # local port to the host's loopback listener
openagents ssh remove me@box              # stop a managed host, or detach from an external one
```

Each `--archive OS/ARCH=PATH` is pinned to its SHA-256 when `add` runs. The
store (`--store DIR`, default `~/.openagents/coder-computers`) records the
destination, owner, relay, and archives in `ssh-hosts.json`, so `tunnel` and
`remove` take only the destination. The record never holds the invitation.
`ssh` runs in batch mode and never prompts, so set up keys for the account
first. `--timeout` bounds each command; `tunnel` prints one line when the
tunnel opens and one when it closes, and ends on Ctrl-C.

## Builds on Boat (`openagents boat`)

- `openagents boat run NAME [--size small|default|large] -- CMD` runs CMD
  on the Boat sandbox NAME against this checkout's diff from `origin/main`.
- `openagents boat stop NAME` stops the sandbox; it is free while stopped.
- `openagents boat delete NAME|ID` deletes the sandbox by name or ID.

Set `BOAT_API_KEY`, or use the Secret Manager secret `boat-api-key` through
`gcloud`. Run `openagents boat --help` for the full syntax.

## The GCE pool (`openagents cloud`)

- `openagents cloud up [--hosts N]` grants this computer the GCE spot pool
  `gce` and starts hosts from the daily `oa-coder-host` image until it has
  N; it prints how long each took to be ready.
- `openagents cloud status` lists the hosts, their live runs and idle
  minutes, and the hourly cost estimate.
- `openagents cloud down` deletes every host and revokes the grant.
- `openagents chat work --on gce --issues N,M --parallel K` runs the issue
  flows on the pool, two per host. Hosts delete themselves after 10 idle
  minutes.

Needs `gcloud` signed in to project `openagentsgemini` and `ssh`. See
[`docs/cloud/gce-pool.md`](../cloud/gce-pool.md).

## Reach (NIP-REACH)

Read and edit the owner host directory, read a host's presence, and prove a
route to it. Every command runs through the same live service the Computers
screens use, so the owner authority rule, the freshness verdict, and route
selection match the phone.

```sh
openagents reach directory list --json
openagents reach directory add HOST --label "studio box"
openagents reach directory remove HOST
openagents reach presence HOST            # the sample and its freshness verdict
openagents reach probe HOST               # the encrypted handshake and route class
openagents reach probe HOST --same-machine
```

`directory add` and `directory remove` publish the next directory revision
against the one this device last read. They refuse without the owner key, when
the last read failed, or when two directories share the highest revision.
`presence` exits 1 when the sample is stale or from the future. `probe`
reports the route class: `loopback`, `lan`, `tailnet`, `public`, or `relay`.
It refuses loopback hints unless you pass `--same-machine`, because a host on
another machine cannot be reached at loopback.

`--relay URL` must name a relay one of this device's grants uses; the command
refuses any other relay instead of ignoring it. `--timeout SECONDS` bounds the
relay read and the handshake (default 15).

## Agent Studio (`openagents studio`)

`openagents studio` drives the Agent Studio coordinator
(`coder::task::studio`, [the specification](../verse/agent-studio.md)).
Seats bind a name and role to an auto-start route. A goal starts a lead
task whose reply ends with a plan; the coordinator validates the plan, holds
each entry until the tasks it depends on are done, and then submits it to
the inbox and notes it eligible for the auto-start policy. An invalid plan,
a lead without a plan, and a failed dependency each become a decision on the
goal.

### Launch and tear down

`openagents studio up --repo PATH` launches the studio on a repository in
one command. It admits the repository as a host workspace, turns auto-start
on for the team's routes, seats a team (a lead and two workers on the
signed-in coding agents, or `--team NAME=ROUTE,...` with the lead first),
starts the host when none answers its control socket, and opens Verse in
Everglade. `--no-verse` skips the window, `--no-host` skips starting a host,
and `--sim` opens the simulated team on a scratch repository with no model
spend. `up` remembers its options, so a bare `openagents studio up` opens the
same studio again.

`openagents studio down` stops and undoes only what `up` started and
changed: the Verse window, the host, the seats it added, the auto-start
policy, and the workspace. Anything `up` found already there stays.

`openagents studio host` starts this computer's host the way `up` does,
with no repository, team, or Verse, when none answers its control socket;
`down` stops it. Verse runs it when you confirm starting a host at Alice's
workstation.

### Seats, goals, and memory on this computer

These commands read and write this computer's task store directly
(`--tasks DIR`, default `$OPENAGENTS_TASKS` or `~/.openagents/tasks`, and
`--root DIR`, the host root):

```sh
openagents studio seat set lead --role lead --route codex:gpt-6-luna
openagents studio seat set ada --route claude:claude-opus-5-5
openagents studio seat list
openagents --json studio goal list
openagents studio plan list GOAL
openagents studio memory add "Run cargo fmt before committing." --kind convention
openagents studio lead-review off
```

A route is `PROVIDER[/ENGINE]:MODEL`. A `claude` or `codex` route may name
its engine: `session` runs the seat's tasks in one Claude Code or Codex CLI
session, `loop` runs Microcoder's loop on the provider. The engine
overrides the owner's host-wide `coder.claude` or `coder.codex` setting for
that seat's tasks, and the host refuses a `session` route under any access
but full. `openagents studio lead-review on|off|status` sets or reads whether
the lead reviews a worker's green change before the person's merge decision;
it is on by default.

The resident host runs the same reconciliation with each auto-start sweep;
`openagents studio sync` runs it now.

### Acting on the running host

Every action Everglade's panels take goes through the running host's
control socket as a NIP-HOST `studio.*` operation, so you can work the
studio without Verse. `--control-socket PATH` names the socket; by default
it is the one the OpenAgents app and `openagents host serve --control`
open. `goal submit` and `message` go through the host when one answers, and
use the local task store otherwise.

| Command | What it does |
| --- | --- |
| `status` | Goals, each seat's activity, station, and task, and how many decisions wait. |
| `tasks [GOAL]` | Every task, or one goal's: identity, plan entry, seat, status, and dependencies. |
| `log SEAT` | The seat's log tail. |
| `decisions` | Open questions, approvals with the step they ask to take, and goals waiting on a plan. |
| `answer DECISION [TEXT] [--file PATH] [--always]` | Answer a decision. `--always` approves the step and keeps the standing rule the approval offers for its seat. |
| `review TASK [--diff]` | The task's change at its revisions; `--diff` prints the diff. |
| `merge TASK [--head REV]` | Merge the reviewed change into the checkout's branch. Nothing is pushed. |
| `request-changes TASK TEXT` | Send the change back to the task's seat. |
| `reject TASK [REASON]` | Close the task; its worktree stays until it is archived. |
| `seat pause\|resume\|stop SEAT` | Steer a seat. |
| `task cancel\|retry\|prioritize TASK`, `task reassign TASK SEAT` | Steer a task. |
| `goal submit TEXT --workspace LABEL [--lead SEAT]` | Start a goal. |
| `message SEAT TEXT` | Message a seat, or every seat with `everyone`. |
| `watch [--interval SECONDS] [--limit N]` | The studio, then each change as it happens. |

A task, goal, or decision may be named by a unique prefix of its identity,
such as the 12 characters the tables show. To bind a decision to the change
you read, pass the tree or commit that `review` printed as `--head`; the
command refuses with `stale` when the change has moved since. Under
`--json`, each command prints one JSON document, and `watch` prints one JSON
line per change (a `snapshot` line, then `update` lines). A refusal prints
the host's code and message, and under `--json` it is
`{"error", "code", "operation"}` on stdout with exit code `1`.

```sh
openagents studio goal submit "Add a --verbose flag" --workspace openagents
openagents studio decisions
openagents studio answer 3f2a9c1d0b7e allow
openagents studio answer g1-1ed832be --file plan.json
openagents studio review 5be04d27a1c3 --diff
openagents studio merge 5be04d27a1c3 --head 8c1e0f4
openagents --json studio watch
```

## The workshop agent (`openagents agent`)

`openagents agent` is a client of the workshop agent, Alice
([the specification](../verse/workshop-agent.md)). The running host plans,
checks, and journals every request; this command sends NIP-HOST
`studio.agent.*` operations over the host's control socket, as Everglade's
desk panel does. A read falls back to her records in the host root when no
host answers. Making an agent, attesting her key, retiring her, and adding
or turning on a standing job are the owner's actions at the host, so they
write the host root (`--root DIR`, default `~/.openagents/host`) directly.

```sh
openagents agent new alice --workspace ~/code/openagents --owner-key owner.key
openagents agent ask alice run the atif tests and tell me what fails --wait
openagents agent answer alice confirm
openagents agent ask alice fix the typo in the README --mode task --workspace openagents
openagents agent show alice
openagents agent log alice
openagents agent memory alice note mobile is its own Cargo workspace
openagents agent memory alice accept 3
openagents agent jobs alice add nightly-check
openagents agent jobs alice on nightly-check
openagents agent stop alice --reason "enough for today"
openagents agent resume alice
openagents agent retire alice
```

`new` makes her own Nostr key in her directory (`key`, mode `0600`) and,
with `--owner-key FILE`, records the owner's NIP-OA attestation of it for
`--days N`, at most 365. `stop` runs the kill switch and journals each step;
`pause` keeps everything and starts nothing new. In the smart terminal,
`@alice TEXT` on the input line sends the same request as `ask`. A task-mode
change waits at the Merge station; `openagents studio review TASK` and
`openagents studio merge TASK` act on it from here.

Sales presets (Paul, Erin, Frank, Pat, Arthur, and Vanna) use the same
identity and private memory. Their creation, charter edits, and signed
verdict recording require the running host's owner admission; they have no
local mutation fallback. `new NAME --role sales-researcher` selects a sales
job for another name. The initial sales charter permits drafting from the
supplied owner request and the member's own memory, with all model tools,
workspace reads, task execution, and autonomous jobs disabled.
`agent charter NAME --role ROLE --expected N --drafting off --purpose TEXT`
narrows the current charter. `agent verdict NAME record FILE` retains a
typed, signed owner-recorded recommendation; `agent verdict NAME list`
reads up to 24 retained records. Evidence and question-set digests are
references, never approval or proof of an independent model decision.
Live sales activation remains in [NEEDS_OWNER.md](../../NEEDS_OWNER.md#sales-outreach-launch).

## Verse (NIP-MV)

Headless presence: see who is around, listen, speak, move, and gesture.
Every event is verified before it counts, and stale states never overwrite
newer ones.

```sh
openagents key show                     # this identity's public key (creates it)
openagents verse who --json             # every entity with a state, nearest first, with names
openagents verse name Alice             # the name over this key's head (24 drawable chars)
openagents verse look --at 0,0,0 --radius 1 --wait 5
openagents verse move 3,0,-2 --yaw 90 --name devin
openagents verse say "hello" --to near
openagents verse gesture greet --to PUBKEY,avatar
openagents verse tail --wait 60 --json  # poses, gestures, states, chat as they arrive
openagents verse leave
openagents verse walkers 20 --world verse-bare --wait 60 --json   # 20 simulated Grid players
openagents verse load --world verse-bare --players 20 --wait 60 --json   # what a viewer sees of them
```

`verse walkers` walks N fresh-key players in loops in front of the Grid spawn,
at the shared 5 Hz cadence or `--hz RATE`; `--loopback` starts an in-process relay.
`verse load` listens to every pose frame in the world and reports each
publisher's rate, gaps, and frame age; it exits 1 when fewer than `--players`
publishers sent a frame. The retained 20-player receipt against the public relay
is `docs/audits/receipts/2026-10-05-grid-walkers-20-public-load.json`.

`--relay` defaults to `wss://relay.openagents.com`, `--world` to
`verse-plaza`, and `--as` names the profile key (default `default`).
`--world` also takes a zone's name (`everglade`, `lagrange-1`, `plaza`),
which stands for that zone's shared world (`verse-everglade`,
`verse-lagrange-1`); `walkers` defaults to `verse-bare`, the Grid.

### Driving the in-world terminal (`verse terminal`)

The Verse window on this computer listens on
`~/.openagents/verse/terminal.sock` (`VERSE_TERMINAL_SOCKET` overrides it;
`--socket PATH` names another). `openagents verse terminal` sends one JSON
request a line to it and prints the reply, so a shell, a test, or an agent
drives the same panes the window draws. Nothing crosses the relay: the panes
are PTYs of this computer, and the socket admits only this user.

```sh
openagents verse terminal status --json          # open, focused, tabs, panes with ids and sizes
openagents verse terminal open                   # show it with focus; the first pane starts
openagents verse terminal split cols -- /bin/sh          # a shell pane beside the first
openagents verse terminal send 'echo terminal-$((40+2))' --enter
openagents verse terminal read --wait-for terminal-42 --json   # exits 1 when it never appears
openagents verse terminal key ctrl-c
openagents verse terminal focus left             # or a pane id, right, up, down
openagents verse terminal tab new; openagents verse terminal zoom
openagents verse terminal close; openagents verse terminal hide
```

Requests are served between frames, so a reply means the overlay did it; a
request waits up to 10 s for a window that has stopped drawing. `send`
pastes its text through `coder_vt::Terminal::paste` (bracketed when the
program asked for it); `key` encodes a named key or chord through
`Terminal::key`, as the keyboard does.

### Authoring townsfolk (`verse town`)

`openagents verse town` reads and stages Everglade's villager definitions
in `crates/verse-zone-everglade/townsfolk/` of the checkout it runs in
(`--dir PATH` names another). Anyone may run `list`, `validate`, `preview`,
and `propose`; only the owner runs `admit` and `remove`, at a terminal.
[Authoring townsfolk](../verse/generative-agents.md#authoring-townsfolk-for-bob)
has the file format, the checks, and the budgets.

```sh
openagents verse town validate --json                  # every definition; exits 1 on a problem
openagents verse town preview mira-baker --at 07:30,12:30
openagents verse town propose mira-baker               # writes townsfolk/proposals/mira-baker.json
openagents verse town admit mira-baker --owner         # asks you to type the ID
```

### Driving owned entities (`verse control`)

NIP-MV entities belong to the key that signs them, so `control` drives
only entities this identity publishes — for example an agent it spawned.
It sends the same state, frame, and gesture events as `move`, `gesture`,
and `leave`, under the given entity id and role (default `agent`).

```sh
openagents verse control scout-1 move 4,0,4 --yaw 45 --name Scout
openagents verse control scout-1 gesture greet --to PUBKEY,avatar
openagents verse control scout-1 leave
```

### Quests, XP, and the board (NIP-XP)

Read-only. The reader gathers quests (`30193`), awards (`3193`),
revocations (`3194`), and labels (`1985`), fetches the events trusted awards
name, and derives the ledger under the reader's trust list — the same rule
the desktop client's quest board uses. Awards from untrusted referees show
but do not count.

```sh
openagents verse quests --json          # every quest, trusted referees first
openagents verse xp                     # this identity's XP, level, titles
openagents verse xp --pubkey npub1...   # another key's ledger
openagents verse board                  # counts, standings, my level, quests
openagents xp verify-card card.json     # re-derive a trainer card (or an naddr1…)
```

`openagents xp verify-card` (also `openagents verse xp verify-card`) reads a
signed trainer card (NIP-XP `30194`) from a JSON file, standard input
(`-`), or an `naddr`; checks its signature; derives the ledger from the
card's relay (or `--xp-relay`) under the card's own trust list; and compares
the keys linked both ways, the counted awards, the XP, and the level under
`trainer-curve-v1`. It prints every difference and exits 1 when there is
one.

`--xp-relay` chooses the XP relay (default `VERSE_XP_RELAY`, then the world
relay); `--referee KEY` trusts another referee for this reading only.
A fresh install trusts the shipped OpenAgents referee. Use
`openagents verse trust list`, `openagents verse trust add KEY`, or
`openagents verse trust remove KEY` to manage referees without editing files.
The first change saves an explicit list in
`~/.openagents/knowledge/xp-trust.json`; that list replaces the defaults,
so removing the OpenAgents referee stays effective across restarts.
Portals, replays, and captures stay desktop-only: they are local
demonstrations with no event on the wire to drive.

## Chamber (`openagents chamber`, the authoritative world)

The chamber is the host-owned 30 Hz world the Verse desktop plays combat in
(`verse-world::service`, [networking](../verse/networking.md)): TLS over
TCP, framed JSON, every command signed by the player's key and admitted
by the host's enrollment list. NIP-MV presence (`openagents verse`) says
who is in the world; the chamber says where their adventurer is, what it
hit, and what it carries. `openagents chamber` hosts one and plays in one
from the command line:

```sh
openagents chamber pack ~/chamber/assets           # the compiled ritual pack
openagents chamber tls ~/chamber                   # cert.der and key.der
openagents key show --as primary                   # a key to enroll
openagents chamber host ~/chamber/host.json        # until stopped
```

`host.json` is the host's configuration: `listen`, `instance`, `scene`
(`assets/verse/original/ritual.json`), `pack`, `certificate_der`,
`private_key_der`, and `enrollments`, each a public key with a role
(`{"type":"primary"}`, `{"type":"player","spawn":[3,0,-22]}`, or
`{"type":"spectator"}`). `state_dir` makes the host durable across
restarts. The host prints a `listening` line with the content identity its
clients must present.

Clients name the host with `--to HOST:PORT --instance N --trust cert.der
[--server-name NAME] [--content HEX]`, or put the same fields in a file and
pass `--chamber FILE`; `--as PROFILE` picks the key (the enrolled one):

```sh
openagents chamber status --chamber client.json --as player     # admission, tick, actors
openagents chamber move 0,1 --ticks 30 --chamber client.json --as player
openagents chamber jump --chamber client.json --as player
openagents chamber cast magic-missile --target 2 --chamber client.json --as player
openagents chamber cast fireball --aim 1,-1 --chamber client.json --as player
openagents chamber respawn --chamber client.json --as player
openagents chamber inventory --chamber client.json --as player
openagents chamber use ITEM | equip main-hand ITEM | equip outfit ID
openagents chamber quest accept ID --giver ACTOR | quest claim ID
openagents --json chamber events --after 0 --limit 64 --chamber client.json --as player
openagents --json chamber watch --wait 30 --hz 2 --chamber client.json --as player
```

Every refusal is the host's own rule, printed as it was sent: a command
before the scene's opening cut ends, a cast while another is in flight, a
target that is not a living hostile, a respawn while alive, and a row-two
catalog spell from a remote player (the host serves those only to its
local caster). A spectator enrollment reads `status`, `snapshot`, `events`,
and `watch` and holds no adventurer. `events` pages the committed authority
log (dialogue, camera handoff, damage, death, respawn) by serial, and
`watch` prints snapshots and new events as NDJSON until the wait ends.
### A public chamber with guests

A host admits any key that proves itself when its configuration carries a
`guests` policy beside (or instead of) `enrollments`:

```json
"guests": { "cap": 16, "ring": [0, 0, -22], "radius": 3 }
```

Each new key becomes a player on the spawn ring, up to `cap` guest players
(configured players and guests together stay within 63); a returning key
keeps its adventurer. A full ring refuses with `Chamber guest capacity
exceeded`, a blocked ring spot moves the spawn to the next free one, and
spectators stay enrollment-only. A key holds one seat however often it
reconnects, and the listener's budget of eight connections per address
bounds how many guest seats one address can take at a time. Saved state recovers guests as
players; removing the policy later refuses recovery, as any changed rights
do.

`openagents chamber service install CONFIG.json [--binary PATH] [--state DIR]`
runs that host from login on as a launchd agent (macOS) or systemd user unit
(Linux) named `com.openagents.chamber`, logging to
`~/.openagents/chamber-host.log`; `service status` and `service uninstall`
report and remove it. Against a public instance,
`openagents --json chamber status --to HOST:PORT --instance N --trust cert.der`
works with any profile and reports `population.players` (admitted player
seats besides the chamber's own first seat) and `population.alive`.

### A chamber over REACH

A chamber can instead run beside a Coder host and admit devices by their
NIP-HOST grants, with no certificate or enrollment list. Set
`"transport": {"type": "reach"}` in `host.json` (add `"websocket": true` for
a WebSocket listener) and drop `certificate_der` and `private_key_der`.
`enrollments` becomes only the role table: a listed key gets its role, and
any other device whose grant holds the `world` right joins as a spectator.
Grant `world` explicitly; no rights preset includes it:

```sh
openagents host invite --rights observe,world        # or: computer invite HOST --rights observe,world
openagents chamber host ~/chamber/host.json          # the host's own store and key
openagents chamber host ~/chamber/host.json --keys "$HOME/.openagents/connect" --label "Ritual"
```

The chamber opens the same access store and host key as `openagents host
serve`, with the same `--state DIR`, `--root DIR`, and `--keys DIR` or
`--keychain` options, and refuses a store the Coder host never initialized.
The channel names the instance number as its generation. When the key
source also holds the owner key, as the desktop app's does, the chamber adds
the instance to this host's existing entry in the owner directory on the
first relay the host root records, and prints a `directory` line either
way. It never adds the host itself.

A device joins with its own key and grant from the computers store
(`--store DIR`, default `~/.openagents/coder-computers`):

```sh
openagents chamber status --to 192.0.2.10:7400 --instance 170 --reach HOST
openagents chamber move 0,1 --to 192.0.2.10:7400 --instance 170 --reach HOST --websocket
```

`HOST` is the Coder host's key, alias, or label. A grant without `world`,
a revoked grant, or an old epoch is refused at the handshake, and a
revocation closes an open connection within seconds.

With `"profile": "everglade"` in `host.json`, the chamber hosts Everglade
under the social rules instead of combat. The scene must hold only friendly
actors, with no cues and `cut_at` 0. The chamber reads the Agent Studio of
the Coder host on the same computer over its control socket
(`--studio-socket PATH` names another) and walks the studio's seats for
every viewer. `chamber status` then reports a `social` object with the seat
poses and, over REACH, the `panels` this device's grant opens: `world` alone
opens none. Desktop Verse built with the `remote-chamber` feature joins with
`verse --join FILE`, where the file names `address`, `instance`, `host`,
`store`, and optionally `content` and `websocket`.

## Zone (Lagrange construction)

The Lagrange zone is a pure simulation (`crates/verse-lagrange`), so the
command runs it headlessly and applies verbs in order:

```sh
openagents zone info                     # landmarks, parts, slots, limits
openagents zone run fly depot grab install status
openagents zone run fly depot grab fly jig release wait 2 --trace
openagents zone build --json             # every part from depot to jig slot
```

Verbs: `fly X,Y,Z`, `fly depot|jig|airlock|spawn|PART`, `grab`, `install`,
`release`, `unclip`, `clip`, `stop`, `wait SECONDS`, `status`, `parts`.
`unclip` lets the safety tether go and `clip` clips it back on within 3 m
of its clip. `install` carries the
held part to its slot, correcting for the carry offset, and fails if it does
not latch. The speed, thrust, tether, and latch limits are the simulation's
own.

To drive a zone another client simulates, `openagents zone send VERB...
--to OPERATOR_PUBKEY` publishes the verbs as NIP-MV kind `23302` zone
commands, and `openagents zone listen` prints the commands addressed to
this identity. The `23302` kind is specified in `nips/openagents/NIP-MV.md`.

The Verse desktop client is an operator for the zone it has loaded. It
admits commands from its own key and from the keys listed one per line in
`<VERSE_HOME>/zone-operators` or comma-separated in `VERSE_ZONE_OPERATORS`
(64-character lowercase hex; `#` starts a comment). It applies `fly`,
`grab`, `release`, `unclip`, `clip`, `stop`, `status`, and `parts` to the
loaded Lagrange station and answers each command with a `zone-ok` or `zone-refused`
gesture, which `zone send` waits for. Commands for another zone, from an
unlisted key, or naming `install` or `wait` (headless-only verbs) are
refused without touching the simulation.

## Sovereign agents (NIP-SOV)

```sh
openagents sov status
openagents sov profile new NAME --agent PUBKEY --authority PUBKEY --policy REF \
  --custody-adapter openagents.local-key.v1 --custody-adapter-artifact REF \
  --custody-policy REF --state-schema REF --disclosure REF
openagents sov admit NAME --as AUTHORITY_PROFILE
openagents sov spawn NAME --seconds 600 [--tick 5] [--ticks N] [--key PROFILE] [--name TEXT]
openagents sov status NAME
openagents sov stop NAME
openagents sov list
```

`sov` implements the first two steps of NIP-SOV's implementation order: the
pure contracts and a bounded local lifecycle under a no-spend, explicitly
trusted local custody profile. `admit` signs the exact profile bytes with
the authority's key (a local key profile whose pubkey is the profile's
`authority`) and writes the record beside the profile; the profile must pin
the `openagents.local-key.v1` custody adapter and name no treasury and no
guardian policy. `spawn` checks, in order, the profile, the admission, the
agent key (the key profile named by `--key`, default `NAME`, must hold the
profile's `agent` key), the policy, the finite budget (`--seconds` is
required), the exclusive controller claim, the environment, and the
checkpoint store; the first refusal stops it with nothing started. When all
pass it writes `NAME.activation.json` and starts the lifecycle in its own
process: each tick publishes the agent's NIP-MV state, so it appears in
`openagents verse who` with role `agent`, and writes one checkpoint revision
under `NAME.checkpoints/`. The plan ends at its budget, on `sov stop`, or
when the relay refuses, publishes the agent offline, and records the reason
in the activation. Treasury, guardians, and portable recovery are not
implemented; `sov status` lists them as missing.

## Gym (NIP-EVAL)

`eval` is a view over `crates/gym`: it scores doors against a pinned suite,
reads a receipt-chained store back as a record, and judges the store's sides
under a gate. It changes no suite, gate, or row schema.

```sh
openagents --json eval run --door lev=http://127.0.0.1:8081 --timeout 30 --record rows.jsonl
openagents --json eval report --store rows.jsonl --suite suite.json
openagents --json eval compare --store rows.jsonl --baseline lev --gate probability-v2
```

`run` asks every door each item of the calibration and development
partitions; `--partition` narrows to one. The locked partition is never
scored by a flag. `--timeout` bounds each door call and is required. An
item a door fails to answer leaves no row and is listed under `lost`, never
scored as wrong; a run with lost items reports `"complete": false` and
exits 1. `report` refuses a broken receipt chain and says when
coverage is undeclared (no `--suite`). `compare` refuses to judge two sides
that did not score the same items.

`gym` is the client half of `crates/gym-bridge`: a separately granted
connection to a private Gym host. Observing never starts work, and a launch
sends only an admitted recipe id, its exact revision, and a durable request
id.

```sh
openagents gym connect --file connection.txt --as default
openagents --json gym status
openagents --json gym observe --relay wss://relay.example --timeout 20
openagents --json gym launch RECIPE_ID --confirm --relay wss://relay.example --timeout 20
openagents gym forget
```

`connect` verifies the `gym-connect:` code against the profile's key
without touching the network and keeps it at
`~/.openagents/gym/PROFILE.connection` with mode `0600`
(`OPENAGENTS_GYM_HOME` overrides the directory). The code is never printed.
`--relay` is required for `observe` and `launch` and must equal the relay
the grant names; a different relay is refused rather than substituted.
`--timeout` bounds the whole exchange; the bridge itself bounds each relay
round trip to eight seconds. `launch` without `--confirm` prints the recipe
and exits 1 with nothing sent. A recipe or revision outside the grant is
refused before anything reaches the relay. When a launch gets no confirmed
reply, the JSON carries `"disposition": "unknown"` and the `request_id` to
retry with `--request-id`, so the host cannot be asked twice by accident.

## Labor (NIP-MKT, NIP-LAB)

```sh
openagents labor offer NAME setup.json [--relay URL --timeout 10] [--as PROFILE]
openagents labor order NAME rfq.json quote.json order.json order_ack.json
openagents labor deliver NAME EVENT_ID --relay URL --attach artifact.json
openagents labor accept NAME acceptance.json
openagents labor execute NAME execute.json --grant grant.json [--tasks DIR]
openagents labor check NAME [--reconcile]
openagents labor list
```

`labor` exposes `crates/coder-labor`: a book is one operator-admitted setup
(market, offering, encrypted terms, and the pinned closure) with a private
journal under `~/.openagents/labor/NAME/` (`LABOR_HOME` overrides). `offer`
admits the setup and, with `--relay`, publishes the signed offering. `order`
applies NIP-MKT negotiation records, `deliver` applies NIP-LAB linkage,
submission, delivery, verification, review, and dispute records, and `accept`
applies the buyer's acceptance. `execute` dispatches the bound CJ request under
the operator's local grant or reconciles the dispatch that already exists.
Every EVENT is a file, inline JSON, `-` for stdin, or a 64-hex event ID
fetched from `--relay` within `--timeout`.

Refusals exit 1. Under `--json` the document carries `error` and a typed
`reason`: `admission` (the setup's closure or terms differ from what this host
admits), `transition` (a record was refused or the evidence conflicts),
`store`, `relay`, or `execution`. Nothing in the group authors a record, widens
a grant, or retries an execution.

Paid books additionally pin canonical partner custody and a separately signed
current provider policy. `execute` and `verify` require explicit `--pipeline DIR`,
`--credential FILE`, and `--authority-evidence FILE` (a private `Blobs` document).
`verify NAME` runs the protected buyer checker; passing still requires separate
signed review and acceptance. `support NAME EVENT` retains a signed cancellation,
zero-revision rework refusal, support action, or known/unknown cost report, and
queues narrowing notices while the existing runner holds the journal.

`invoice NAME` uses the same authority options plus `--wallet-home DIR` to issue
one exact postacceptance invoice on the admitted resident node. The buyer
separately authorizes that invoice through `openagents x402 node pay BOLT11
--max-fee-msat N` using the accepted ceiling. `fund NAME --ledger FILE` additionally
uses authenticated inbound lookup and the existing private central ledger to
accrue one accepted worker share. It sends no payout and opens no replacement
wallet. Unknown outcomes remain unknown, and later cancellation preserves
already accepted obligations. `check` includes the private paid state, support
outcomes, and incomplete all-in costs. The selected scope has zero executable
revisions or reworks, no escrow, and no inferred FX or added platform fee. See
[explicit paid fulfillment](../coder/runtime/free-labor.md#explicit-paid-fulfillment)
for the bounded contract and owner qualification.

## Your wallet (`openagents wallet`)

`openagents wallet` is the person's Spark wallet on this computer: the same
wallet, from the same seed, as the OpenAgents app on the phone, so there is
one balance on every device (owner decision 8 in
[docs/breez/README.md](../breez/README.md#owner-decision-2026-10-02)). It runs
on Bitcoin mainnet only, through the shared `crates/spark-wallet`.

```sh
openagents wallet create            # a new wallet, for a person or agent without the phone app
openagents wallet link              # bring the phone's wallet here (approve on the phone)
openagents wallet restore           # or type the recovery words (not shown as you type)
openagents wallet balance           # Your balance is ₿12,000 (0.00012000 BTC).
openagents wallet address           # Your address: spark1…
openagents wallet receive --amount 5000   # a Lightning invoice to share
openagents wallet receive --bitcoin       # a Bitcoin address to send to
openagents wallet send alice@example.com --amount 1000   # shows the fee, asks before paying
openagents wallet history
```

Answers are plain: a balance is one sentence and an address one line, and
none names a node, a network, a chain server, channels, or liquidity.
`--json` keeps machine fields (`balance_sats`, `address`, `invoice`,
`payments`). `send` asks before it pays (type `yes`); `--yes` pays without
asking, and a non-interactive `send` without `--yes` pays nothing.

`link` needs this computer's OpenAgents host, which the phone reaches over
NIP-HOST. The command makes a one-time key, prints a six-digit code, and
records a request beside the host's access store; the app on the phone
shows "Use your wallet on COMPUTER?" with the same code. Only after the owner
approves there (with Face ID or the passcode) does the phone seal the seed's
entropy with NIP-44 to the one-time key, which only the waiting command
holds. The host and relay see ciphertext; the request is removed once read.
`restore` reads 12 or 24 recovery words with echo off (or one line from a
pipe). Both refuse to replace a different wallet without `--replace`.
`create` makes a new seed and shows its 12 words once: on a terminal, until
the person types `saved`; elsewhere only with `--show-words`. Nothing else
prints the seed or the words. More in [wallet.md](wallet.md).

Files live in `~/.openagents/spark` (`OPENAGENTS_SPARK_HOME` overrides): the
seed as hex entropy in `seed` (mode 0600, folder 0700), and Breez's records
as one JSON file per wallet under `wallets/`. The computers' Cargo workspace
cannot link Breez's SQLite store beside `ldk-node`'s, so computers keep the
records in that file; the phone keeps Breez's SQLite store.

## The x402 node (`openagents x402 node`, the Lightning rail)

`openagents x402 node` runs an embedded [ldk-node](https://github.com/lightningdevkit/ldk-node)
Lightning node from `crates/wallet`. Its node key is held only by this
wallet, so its node id is a valid x402 `payTo`; it issues the exact-amount
invoices NIP-X402 receivers need and pays them as a buyer.

```sh
openagents x402 node init --network testnet     # bitcoin, testnet, signet, or regtest (--esplora URL)
openagents x402 node info --json                # node_id is the x402 payTo
openagents x402 node fund                       # on-chain address to fund channels from
openagents x402 node channel open NODE_ID@HOST:PORT --sats 100000
openagents x402 node channel list
openagents x402 node invoice --msat 1000 --request-hash HEX64 --json
openagents x402 node pay BOLT11 --max-fee-msat 50 --wait 60 --json
openagents x402 node lookup PAYMENT_HASH
openagents x402 node backup DIR                 # seed + config + store snapshot, digests in backup.json
openagents x402 node restore DIR                # into an empty wallet home
openagents x402 node channel close USER_CHANNEL_ID COUNTERPARTY [--force]
openagents x402 node serve                      # the resident node: events as JSON lines, answers control.sock
openagents x402 node service install            # run `x402 node serve` from login on (launchd or systemd --user)
```

`x402 node serve` is the resident node. It binds `control.sock` in the wallet
home and answers every other wallet command, and every `x402` command that
needs the node, over that socket; `info` then reports `resident` with the
serving pid and uptime, and a payment through it completes in well under a
second. A command that finds no resident opens the node itself, acts, and
stops it, which costs a chain sync each time and leaves nothing online to
receive on. A second `serve` under the same home is refused while the first
answers; a socket left by a killed resident is replaced. The resident dials
its stored channel peers every 5 seconds until they connect, so a
counterpart that comes online is usable within seconds rather than the
node's own one-minute retry. `x402 node service install` writes a launchd
agent (`com.openagents.wallet`, `gui/UID`) or a systemd user unit that runs
`openagents --json x402 node serve` for this wallet home with restart on exit,
and `service status` reports both the manager's view and the resident that
answers.

`invoice` puts the request hash in the BOLT11 description hash (`h`) and
uses no memo, so `crates/nostr::x402` accepts it for that amount, hash, and
`payTo`. `pay` refuses a malformed, foreign-network, expired, or self-issued
invoice before dispatch, holds the router to `--max-fee-msat`, and prints the
32-byte preimage plus the fee as proof; paying the same invoice again returns
the same proof, and a payment still pending after `--wait` exits 1 with the
payment hash for `lookup`. An unpaid inbound preimage is never shown.

Files live in `~/.openagents/wallet` (`OPENAGENTS_WALLET_HOME` overrides):
`config.json`, the `seed` (mode 0600, printed only by `export --reveal`),
and the `ldk/` store. The node needs an Esplora server to start, so every command except `init`
needs the network. The x402 validator admits only mainnet and testnet invoices
(`bc`, `tb`), so signet issues but does not validate. `openagents x402
advertise` publishes the NIP-CAP `oa-x402-v1` head for a paid endpoint.

### Inbound liquidity from an LSP

A fresh node can pay but not receive until a peer has capacity toward it.
`init --lsp` names a liquidity provider. `--lsp mdk` picks the MoneyDevKit
LSPS4 peer for the wallet's network (bitcoin, or signet on Mutinynet, where the
preset also sets the Mutinynet Esplora server); `--lsp olympus` picks the
Olympus (ZEUS) LSPS1 peer (bitcoin or testnet); and a peer given as
`NODE_ID@HOST:PORT` speaks LSPS2 unless `--lsp-protocol lsps1|lsps4` says
otherwise.

Two of these work for x402. With LSPS4, `invoice` asks the LSP for a
just-in-time route hint and this node builds and signs the invoice itself, so
the first payment opens the channel and the node id stays a valid `payTo`; no
funding step comes first. With LSPS1, the LSP opens a channel in advance for a
fee (`channel buy`). An LSPS2 or Olympus Flow just-in-time channel wraps the
first payment in an invoice the LSP signs, which the x402 payee check refuses.
Once a usable channel has enough inbound capacity, `invoice` issues an
ordinary invoice and no LSP is involved. `--lsp-token` carries an LSPS4 fee
claim or an LSPS2 token; `--lsp-min-msat N` records the smallest payment the
LSP forwards (from its published fee policy), and x402 providers then refuse a
toll below it. The LSPS4 provider takes its fee from the forwarded amount, so
price it into the toll.

The LSP holds the first payment only for a short time while it opens the
channel (45 s in MoneyDevKit's service). Against the staging LSP on Mutinynet
the channel opened but took about 60 s, so the LSP failed the payment back with
`unknown_next_peer`, and the same invoice stayed unpayable afterward. The
channel stays open, so the buyer's next challenge gets an ordinary invoice that
settles at once; x402 buyers retry a failed payment with a fresh challenge.

```sh
# Signet on Mutinynet against MoneyDevKit's staging LSP.
openagents x402 node init --network signet --lsp mdk
openagents --json x402 node invoice --msat 21000000 --request-hash HASH
# {"bolt11":"lntbs210u1…","pay_to":"<this node id>","description_hash":"HASH",…}
```

`crates/wallet` builds on MoneyDevKit's fork of `ldk-node` 0.7 (pinned by
revision), which adds the LSPS4 client; upstream `ldk-node` 0.7 has none.

```sh
openagents x402 node init --network bitcoin --lsp olympus
# Quote 200 000 sats of inbound capacity; nothing is paid yet.
openagents --json x402 node channel buy --lsp-sats 200000
# {"order_id":"a4ac…","bolt11":{"state":"expectpayment","fee_total_sat":7728,
#   "invoice":"lnbc77280n1…"},"onchain":null,"channel":null,"paid":null}
# Pay the order from this wallet and watch for the LSP to fund it.
openagents --json x402 node channel buy --lsp-sats 200000 --pay lightning
openagents --json x402 node channel order ORDER_ID
```

Against Olympus mainnet a 200 000 sat order quoted 7 728 sats, three
confirmations required, and funding within six blocks, and offered BOLT11
only (`onchain` was null). Olympus therefore expects the fee over
Lightning, so a node with no channel first funds itself (`fund`), opens an
outbound channel to Olympus with `channel open 031b30…@45.79.192.236:9735
--sats N`, and pays the order through it; `--pay onchain` and `x402 node send
ADDRESS --sats N` serve LSPs that quote an on-chain address. The order document
also reports `client_balance_sat` (`--our-sats`, sats the LSP pushes to this
side), `channel_expiry_blocks` (`--expiry-blocks`, default 13 000, about 90
days), and `channel` once the LSP has funded the channel.

### Backup and restore

The seed and the channel store are two different recovery assets. The
seed alone recovers every on-chain coin and the node id. Channel funds
also need `ldk/ldk_node_data.sqlite`: the counterparty holds the only
other copy of each channel's state, and a node that runs from an older
copy of that store can broadcast a revoked commitment and lose the
channel's balance as a penalty.

```sh
openagents --json x402 node backup ~/wallet-backups/2026-09-27
# {"path":"…","created_at":1790567142,"store":true,"resident_running":true,
#  "files":[{"path":"seed",…},{"path":"config.json",…},{"path":"ldk/ldk_node_data.sqlite",…}]}
openagents --json x402 node info                    # last_backup: {at, path, files, bytes}
openagents x402 node export --reveal                # the mnemonic, on stdout, nothing else
openagents x402 node export                         # refused: exit 64
```

`backup DIR` writes the seed, `config.json`, and a consistent snapshot of
the store (SQLite `VACUUM INTO`, so it is safe while the resident is
running) into a new private directory, with a `backup.json` manifest that
records the SHA-256 of every file. It refuses a directory that already
holds a backup, and records the time, path, and size in
`last-backup.json` in the wallet home, which `info` reports as
`last_backup`. Back up after every channel open or LSP order.

Two ways back, into an empty wallet home:

```sh
# Full restore: seed, config, and the channel store.
openagents --json x402 node restore ~/wallet-backups/2026-09-27
# Seed only: the mnemonic on stdin, never on the command line.
cat mnemonic.txt | openagents x402 node init --network bitcoin --mnemonic -
```

`restore` verifies every digest against the manifest before it copies
anything and refuses a home that already has a seed. After a full
restore, never run the old copy again: two nodes on one store is the
revoked-commitment case above. Start the restored node once (`x402 node
info` or `x402 node serve`) and it resyncs; channels opened after the
backup are unknown to it and fall under the seed-only rule.

A restore from the seed alone has the same node id (a
`restored_wallet_keeps_its_node_id_and_addresses` test pins node id and
funding addresses across both restore paths) but no channels. The
funds in each channel then come back only when the counterparty
force-closes: reconnect to it and ask it to close, or if the peer is
another `openagents x402 node`, run `channel close USER_CHANNEL_ID
COUNTERPARTY --force` there. LDK's on-chain sweeper on the restored node
claims its side of the closing transaction once it sees it. `channel
close` without `--force` is the cooperative close for a channel this
node still knows, and needs the peer online.

## Paid HTTP (`openagents x402`, exact Lightning over `http:1`)

`openagents x402` sells one command over HTTP for an exact millisatoshi
price, or buys one call. The seller side is the embedded facilitator from
`crates/x402`: it challenges, reconstructs the request binding, verifies the
proof, consumes it in a replay store, then runs the command.

```sh
# Seller: every POST to /echo costs 1000 msat and runs `cat` on the body.
openagents x402 serve --url https://host.example/echo --msat 1000 \
    --listen 127.0.0.1:8402 --seconds 3600 -- cat

# Buyer: pay at most 1000 msat (+ 50 msat routing fee) for one call.
echo hi | openagents x402 fetch https://host.example/echo --method POST --body - \
    --max-msat 1000 --max-fee-msat 50 --json
```

The wire shapes and headers follow x402 v2 at commit `4fcf836`:
`PAYMENT-REQUIRED` carries the base64 `PaymentRequired` document with one
`exact`/`lnbtc` requirement whose `requestHash` binds the method, the
configured public URL, and the body bytes (`http:1`, no bound headers), and
whose `extra.invoice` is a fresh BOLT11 from the wallet with that hash in
`h`. `PAYMENT-SIGNATURE` carries the buyer's `PaymentPayload` with the
preimage; the server never trusts the payload's own request hash and
reconstructs the binding from the request it received. `PAYMENT-RESPONSE`
carries the `SettlementResponse`: on success `transaction` is the payment
hash; on refusal `errorReason` uses the upstream vocabulary
(`invalid_exact_lnbtc_invoice_request_mismatch`,
`invalid_exact_lnbtc_preimage_hash_mismatch`, `duplicate_settlement`, ...).

Settlement consumes `network:payment_hash` in `~/.openagents/x402/replay`
(`OPENAGENTS_X402_HOME` overrides) with an exclusive file create, so two
concurrent presentations of one preimage admit exactly one; records stay
until `invoice_end + skew + 3600`. Payment happens before execution: a
command that then fails returns 500 with the settlement header and no
refund, as NIP-X402 specifies. The buyer binds the request itself, accepts
only a requirement whose invoice validates for that binding, refuses above
`--max-msat` before paying, does not follow redirects, and prints the
preimage only with `--show-proof`. Neither log line nor `--json` event on
the seller side carries an invoice or preimage.

### Many routes on one wallet (`openagents pay serve`)

`openagents pay serve --routes FILE` is the multi-route front the central
receiver runs ([central receive and splits](../payments/2026-10-02-central-receive-and-splits.md)):
one wallet, one replay store, one settlement log, and every route in a TOML
file. `openagents pay --help` prints the file format.

```toml
public_url = "https://api.openagents.com"

[[route]]
id = "messages"
path = "/v1/messages"
price_sats = 21
role = "endpoint"
command = ["openagents-answer"]          # body on stdin, stdout is the answer

[[route]]
id = "explain-error"
path = "/v1/plugins/explain-error/invoke"
price_sats = 31
role = "plugin_call"
plugin = "explain-error"
plugin_dir = "plugins/explain-error"     # the workflow runs with the body as the request

[[route]]
id = "weather"
method = "GET"
path = "/x/weather/{city}"
price_sats = 5
role = "hosted_resource"
upstream = "http://10.0.0.7:9000/weather/{city}"
```

Every `402` carries one invoice two ways: the x402 terms above, and an HTTP
`Payment` challenge (`WWW-Authenticate: Payment ... method="lightning",
intent="charge"`, with the body's `digest` and the method and URL in its
HMAC-bound `opaque`), so `lnget` pays it as it is. A route priced in msat
that is not whole sats is sold over x402 only. A proof by either scheme is
consumed once across every route; a successful `Payment` answer also carries
`Payment-Receipt`. Before the route runs, the settlement (payment hash,
request hash, route, resource, role, plugin, price, what the wallet received,
scheme, time) is appended to `~/.openagents/x402/settlements.ndjson` and
synced; if that fails the call gets a 503, nothing runs, and the same proof
stays good for a retry. The challenge HMAC key is
`~/.openagents/x402/payment-challenge.key`, made on first use; every process
that settles for this wallet shares it, the replay store, and the log. A
signet or regtest wallet is refused, as for `x402 serve`.

A `registry` plugin route first returns `409` with its current signed release's
`quote` and `quote_digest`. After reviewing the release and separate endpoint
and author fees, submit `{"quote_digest":"<approved digest>","request":"<text>"}`
to obtain an invoice. Retry the same body bytes with proof. Changed terms
require new approval before another invoice; the earlier proof cannot buy
a newer release. The [meeting action-items example](../../plugins/meeting-action-items/README.md)
includes retained publication and paid invocation checks.

### Discovery (`x402 advertise`, `fetch --cap`)

`openagents x402 advertise` publishes the kind `30180` NIP-CAP head that
NIP-X402 specifies for a paid resource: an `adapter` definition with
`requires: ["oa-x402-v1"]`, `remote.endpoint` set to the public URL, and
one `binding_contract.x402` descriptor whose single receiver is this
wallet's node id on the wallet's network. `--binding http:1` (the default)
uses transport `http` and interface `openagents.x402.http.v1`;
`--binding mcp:1` uses transport `mcp`, interface
`openagents.x402.mcp.v1`, and the MCP server URI as the endpoint. The head is
signed by the `--as` key profile; `--dry-run` prints the definition and
event id without publishing. A signet or regtest wallet is refused because
those networks have no x402 network id.

```sh
openagents x402 advertise --test --slug echo --url https://host.example/echo \
    --merchant host.example --summary "echo the body" --json
openagents cap describe PUBKEY:echo        # shows x402 bindings and receivers
echo hi | openagents x402 fetch https://host.example/echo --method POST --body - \
    --max-msat 1000 --cap PUBKEY:echo
```

With `--cap PUBKEY:SLUG`, `fetch` resolves the newest valid head first and
refuses before any request when the URL is not the advertised endpoint, and
after the 402 when the challenge's `network`/`payTo` is not an advertised
receiver pair or the head does not advertise `http:1`. The parser in
`crates/nostr::cap` admits only `oa-x402-v1` in `requires`; any other
feature, the `x402` field without the feature, or the feature without the
field is still a refusal. Native Nostr bindings and recovery contracts
other than `none` are not served yet.

Both sides need a wallet on bitcoin or testnet with a usable channel; the
local check that needs none is a `serve` on a fresh testnet wallet answered
by `curl -X POST` (402 with a bound invoice) and by a forged
`PAYMENT-SIGNATURE` (402, `network_mismatch`).

### Paid MCP (`x402 mcp-serve`, `x402 call`, `mcp:1`)

`openagents x402 mcp-serve` is `openagents mcp serve` with a toll on every
`tools/call`, following the upstream x402 MCP transport unchanged. A call
without `_meta["x402/payment"]` gets a tool result with `isError: true`
whose `structuredContent` is the `PaymentRequired` document and whose
`content[0].text` is the same document as JSON; its one requirement binds
the configured server URI, `tools/call`, the tool name, and the arguments
(`mcp:1`, no bound metadata) into the invoice's `h`. A call carrying a
`PaymentPayload` under that key is settled through the same facilitator and
replay store as `http:1`, runs only after settlement, and returns its result
with the `SettlementResponse` under `_meta["x402/payment-response"]`; a
refused payment is an error result carrying the settlement and no output.

```sh
# Seller: every tool call costs 1000 msat; only the version group is served.
openagents x402 mcp-serve --server mcp://host.example/openagents --msat 1000 \
    --tool version

# Buyer: start the server as a child, buy one `version` call.
openagents x402 call version --max-msat 1000 \
    --server mcp://host.example/openagents -- ssh host.example openagents x402 \
    mcp-serve --server mcp://host.example/openagents --msat 1000
```

`--server` is the URI both sides bind to; neither connects to it. `call`
starts `CMD` as a stdio MCP server, sends the unpaid call, checks that the
challenge's resource is that URI and its invoice validates for this call,
refuses above `--max-msat`, pays, retries with the proof, and prints the
result with `paid`, `amount_msat`, and `settlement`. A server without a
toll answers the first call and `paid` is `false`. With `--cap PUBKEY:SLUG`
the URI defaults to the advertised endpoint and the receiver must be an
advertised pair on a head that lists `mcp:1`. The profiles stay distinct:
an `http:1` challenge is refused by `call`, and an `mcp:1` one by `fetch`.

### Native Nostr (`x402 native-serve`, `x402 buy`, `x402 status`, `nostr:openagents:1`)

The native binding carries a paid run over the relay alone: no HTTP endpoint
and no MCP server. Every record between buyer and provider is a private
kind `3188` artifact, sealed with NIP-44 v2 to the other party and published
over a NIP-42 authenticated connection. The wire records and the ledger
rules live in `crates/x402::native`.

```sh
openagents x402 advertise --test --slug echo --binding nostr:openagents:1 \
    --merchant demo --summary "echo bytes over Nostr" --json
openagents x402 native-serve --slug echo --msat 1000 --seconds 600 --json -- cat
echo -n hi | openagents x402 buy PROVIDER_PUBKEY --slug echo --input - \
    --max-msat 2000 --wait 60 --as buyer --json
openagents x402 status PROVIDER_PUBKEY PURCHASE --as buyer --json
```

`advertise --binding nostr:openagents:1` publishes a head with transport
`nostr-cj`, interface `openagents.x402.native.v1`, recovery
`native-record-v1`, and `remote.worker` plus one to eight `remote.relays`
(`--relay`, repeatable) instead of an endpoint.

`native-serve` listens for `request` records addressed to its key, answers
each with a `challenge` (an invoice whose description hash binds buyer,
provider, purchase nonce, and the request bytes) and an `offered` status,
and runs `CMD` only after a `claim` settles and the durable purchase store
under `~/.openagents/x402/native` admits it. The output is sealed back as a
bytes artifact and the status chain ends in `completed` or `failed`. The
same purchase never runs twice: a second paid invoice is refused as
`purchase_already_admitted`, and a repeated proof as `duplicate_settlement`.

`buy` resolves the provider's head, seals the input, publishes the request,
checks the challenge against the exact request and the advertised receiver,
refuses above `--max-msat` or `--max-fee-msat`, pays exactly once, and
publishes the claim only with payment evidence. It writes a receipt before
paying, then follows the status chain and prints the output only from a
terminal status. `status` replays that receipt: it publishes a
`status_query`, follows the provider's answer, and exits 1 while the purchase
is not terminal. Neither command pays or reruns on a timeout; reconcile with
`status` first. `buy` and the paid `http:1`/`mcp:1` commands refuse each
other's challenges.

First paid round trip on record (testnet, 2026-09-28, relay
`wss://relay.openagents.com`): two `openagents x402 node` nodes on one
machine, a 100,000 sat private channel between them (funding
`329336d0…1944`, block 5151355), provider `native-serve --slug echo
--msat 1000 -- cat` on the receiving node, buyer `buy … --as buyer` on the
other. Purchase `5403a3fb…d1af` went `offered → claim_pending → admitted →
running → completed`; payment hash `bdd4e980…d993`, 1000 msat, 0 msat fee,
and both nodes' `wallet lookup` hold the same preimage (outbound on the
buyer, inbound on the provider). Two things had to be fixed on the way:
`channel open` now stays online until the funding transaction is
broadcast, and `pay` waits up to 20 s for a just-started node to reconnect
its channel peers before it sends.

First paid round trip across machines (testnet, 2026-09-27, #9815): the
provider ran on `coderos-4080` over the tailnet, `wallet serve` resident
(`com.openagents.wallet`, node `02dc6c…b75d`, listening on `9735`) and
`x402 native-serve --slug echo --msat 1000 --as north -- cat`, started and
read back with `openagents computer exec coderos`. The buyer node here
(`03125b…44e7`) opened a 40,000 sat channel to it with `wallet channel open
02dc6c…b75d@100.74.238.61:9735 --sats 40000`; the channel became ready
(`channel_ready` in the resident's log) once both nodes were online after
7 confirmations. `x402 buy 3e7e66…da5ad --slug echo --max-msat 2000 --as
buyer` went `offered → claim_pending → admitted → running → completed`
for purchase `6d9481…7f08`, payment hash `0648b2…d0f8`, 1000 msat, 0 msat
fee; `wallet lookup` on both machines holds preimage `070dba…26eb`
(outbound here, inbound on coderos), and `x402 ledger` here shows the one
entry in phase `completed`.

### Provider recovery (`native-serve --rerun-safe`, `status --list`, `status --finish`)

A provider that stops between admission and finish leaves purchases in
`admitted` or `running` in its purchase store. On start, `native-serve`
scans the store: without `--rerun-safe` each such purchase becomes
`failed` with cause `provider_restarted` and the status is published to
the buyer (`{"event":"recovered",...}`); with `--rerun-safe` a purchase
whose `execute_until` has not passed is queued (`rerun_pending`), its
input artifact is read back from the relay, and `CMD` runs again
(`{"event":"rerun","phase":"completed"}`). `offered` and `claim_pending`
purchases are the buyer's to settle and are left alone.

```sh
openagents x402 status --list --as PROFILE          # open purchases with their windows
openagents x402 status --finish BUYER:PURCHASE \
    --cause operator_cancelled --as PROFILE         # record and publish a terminal cause
```

Accepted causes are `provider_restarted`, `operator_cancelled`, and
`execute_until_passed`. On the buyer side `status` tells a provider that
answered (a terminal phase, exit 0 or 1) from one that has not; in the
second case it says whether the provider's `recover_until` window is still
open, and it never pays again. A `completed` status whose output artifact
was published before `status` subscribed is read back by event id.

Live check on 2026-09-27 (testnet, buyer here, provider on `coderos-4080`
through `openagents computer exec`): purchase `bfffe9…7917` was paid and
left `running` when the provider was killed; `status --list` showed it with
`execute_window_open: true`; a restart without `--rerun-safe` published
`failed/provider_restarted`, and the buyer's `status` ended with that phase
(exit 1, ledger entry `failed`, no second payment). Purchase `1cde4a…ea1d`
was left `running` the same way; a restart with `--rerun-safe` logged
`rerun_pending` then `rerun … completed`, and the buyer's `status` printed
the rerun's output. Purchase `68ca67…12c2` was closed by hand with
`status --finish … --cause operator_cancelled`, after which `--list` was
empty.

### Selling a NIP-CJ worker's output (`native-serve --cj WORKER`)

`native-serve --cj WORKER` replaces `-- CMD` with a NIP-CJ worker: after
admission, the provider signs and encrypts one kind 25900 request carrying
the input as the job's `task`, records the request ID under `run.job` in
the purchase store, publishes the `running` status with that job, and only
then publishes the request to the worker over `--cj-relay` (default
`CODER_RELAY`, then `--relay`). The worker's result text is the output
artifact. Worker outcomes map to stable causes: no bound event before the
contact timeout is `no_worker`, feedback without a result inside the
execution window is `worker_silent`, a typed `status: error` feedback is
`worker_refused`, and any relay or shape failure is `worker_failed`.

```sh
CODER_RELAY=wss://relay.openagents.com openagents x402 native-serve \
    --slug review --msat 5000 --expiry 900 --cj npub1worker... --json
openagents x402 status --list --as PROFILE      # open purchases, each with its job
```

Because the request ID is durable before the job is posted, a provider that
restarts while a job is running logs `following` and waits for that job's
result (`Recovery::Follow`) instead of failing the purchase; a job whose
window has closed fails with `provider_restarted` as before. On the buyer
side, `status` without `--wait` follows a `running` purchase to the end of
its execution window instead of exiting 1 while the worker is still at
work. Input artifacts that are not inline stay out of scope for `--cj`.

Live check on 2026-09-27 (testnet, both wallets on one machine, relay
`wss://relay.openagents.com`, provider `native-serve --slug cjtest --msat
1000 --cj a3d81a…ad91`): with no worker running, purchase `d85bf6…6284`
was paid, `dispatched` job `09972b…4a83`, and ended `failed/no_worker`;
with `coder-worker --once --decline busy`, purchase `a511cc…a6fe` ended
`failed/worker_refused` after the worker logged `declined: busy`; with
`coder-worker --once` on its stub door, purchase `7d35f2…29fc` ended
`completed` and the buyer printed the worker's answer as the output.

### Spending policy and ledger (`x402 policy`, `x402 ledger`)

Every buyer (`fetch`, `call`, `buy`) reads `~/.openagents/x402/policy.json`
(`OPENAGENTS_X402_HOME` overrides the directory) before it pays. The
policy holds a default ceiling, per-provider ceilings keyed by the `payTo`
node id, per-capability ceilings keyed by `PUBKEY:x402/SLUG`, a rolling
24-hour spending cap counted in amount plus fee, and a provider allowlist.
A ceiling has `max_msat` and `max_fee_msat`; the capability entry wins over
the provider entry over the default, field by field, and `--max-msat` or
`--max-fee-msat` on the command line wins over all of them. Without a
`max_fee_msat` anywhere the fee cap is `max_msat / 100 + 1000`. A call
with no ceiling from any source is refused before any request goes out.

```sh
openagents x402 policy set --max-msat 5000 --daily-cap-msat 200000
openagents x402 policy set --max-msat 20000 --max-fee-msat 50 --provider 02dc6c…b75d
openagents x402 policy set --max-msat 1000 --cap PUBKEY:x402/echo
openagents x402 policy allow 02dc6c…b75d       # empty list admits everyone
openagents x402 policy set --max-msat - --provider 02dc6c…b75d   # `-` clears
openagents x402 policy --json
openagents x402 ledger --since 86400 --binding nostr:openagents:1 --json
```

Refusals name the rule: above the ceiling and which source set it, the
provider off the allowlist, or the daily cap the payment would cross.
Nothing is paid on a refusal. Every payment appends one line to
`~/.openagents/x402/ledger.ndjson` — time, binding, network, provider,
capability, resource, amount, fee, payment hash, and phase — and the phase
is updated when the call ends (`http_200`, `completed`, `tool_error`,
`retry_failed`, or the native status name). `x402 ledger` sums and lists it,
filtered by `--since SECONDS`, `--binding`, and `--provider`.

Providers take `--expiry SECONDS` (default 300, `--timeout` still works)
for the invoice and challenge lifetime. `native-serve --per-buyer N`
refuses a buyer's request with `rate_limited` after N requests within an
hour, counted from the purchase store. When the wallet's LSP is recorded
with `wallet init --lsp … --lsp-min-msat N`, every provider refuses to
start with `--msat` below N: buyers could never settle such a toll through
that LSP. There is no pricing discovery or negotiation: the provider sets
its toll, the policy decides whether the buyer pays it.

## Keys and relays

```sh
openagents key list
openagents relay req '{"kinds":[33301],"#w":["verse-plaza"],"limit":20}'
openagents relay tail '{"kinds":[9],"#w":["verse-plaza"]}' --wait 60
openagents relay sign 1 "hello" --tag t=test
openagents relay publish event.json      # a file, inline JSON, or - for stdin
```

`relay` answers NIP-42 challenges with the `--as` profile key.

## Discovery (NIP-CAP, NIP-PRG, NIP-EXT)

By default, `cap list` hides listings tagged `oa:test` or `oa:dev` and
publishers without validated signed NIP-MV avatar or agent state within the
past 7 days. `--all` includes them with an `old/test` column. Mark demo
advertisements with `x402 advertise --test` or `--dev`.

```sh
openagents cap list --profile executor --limit 20
openagents cap describe PUBKEY:SLUG
openagents prg list --step delegate
openagents prg describe --author PUBKEY SLUG
openagents plugin list                     # listings; --type release|revocation|migration|checkpoint
openagents plugin list --package ROOT_PUBKEY:SLUG
openagents discover --origin https://openagents.com
openagents discover --fetch --timeout 5    # compare what the origin serves
```

`cap`, `prg`, and `plugin list` read published heads from one relay (`--relay`,
`--timeout`, and `--as` for NIP-42) and never run a probe, install a
package, or mint a grant. Every record carries `valid` and, when the
signature, kind, marker, or body fails the contract, a `refusal`, so a
malformed head shows up instead of vanishing. `describe` exits 1 when the
head it found is invalid. `discover` prints the agent card and agent-skills
index this checkout serves for an origin; `--fetch` also reads both from the
origin over HTTP and exits 1 when either differs or fails to load.

## Plugins (`openagents plugin`)

A plugin is anything you add to OpenAgents: skills, workflows, knowledge,
Wasm, and the tests that show whether it helps ([plugins](../plugins/README.md)).
`openagents plugin` lists published plugins, tests a plugin with and
without it, publishes and checks results, and syncs the plugins Coder uses
for everyone. `openagents ext` is the older name for the same command, and
`ext eval` for `plugin test`; both keep working.

```sh
cd my-plugin                                         # holds package.json
openagents plugin test init                          # the authoring interview
openagents plugin test init smoke --bare             # a blank evals/smoke/
openagents plugin test run . --trust                 # every test, with and without
openagents plugin test run . --runs 1 --case smoke   # a cheap try
openagents plugin test run . --grant write           # tests that write files
openagents --json plugin test run . out.json         # the report, not the table
openagents plugin test release . --blobs-dir DIR     # release a test set without running it
openagents plugin test publish evals/results/TIMESTAMP/report.json
openagents plugin test check EVENT_ID                # rerun someone's result here
openagents plugin defaults sync                      # the plugins Coder uses for everyone
openagents plugin defaults show
```

`plugin test run` measures whether a plugin changes what Coder does
([the specification](../extensions/evaluation.md), which calls a plugin
test an *extension eval* and a test a *case*). Each case runs as one
`coder -p` turn per arm per attempt: the **subject** arm admits the
plugin's workflow (and the Wasm it carries) through
`CODER_PROGRAMS` and its `skills/*.md` as guidance appended to Coder's
instructions; the **baseline** arm admits nothing. Every run gets a new
`oa-eval-XXXXXX` directory and runs inside `coder-boundary`
(`sandbox-exec` on macOS, `bwrap` on Linux); on any other host every run
refuses as `unconfined_host`. The child sees no variable of your shell: it
gets a door URL and a token for a loopback proxy that holds the real key,
so the key never reaches the child, its trajectory, or the results.

A plugin directory holds its package record in `package.json` (the
`coder::package::Package` record, with an optional `eval_dir`), the
programs it names under `programs/`, and its skills under `skills/`. An
installed identity `PUBKEY:SLUG@VERSION` resolves under
`~/.openagents/extensions/PUBKEY/SLUG/VERSION/`. A directory you haven't
trusted asks once; `--trust` answers yes, and without a terminal it
refuses. `--grant write|exec|network` admits what a case asks for beyond
`read`.

The pinned door comes from this shell: `CODER_DOOR_URL`, `CODER_DOOR_KEY`
(or `CODER_AI_GATEWAY_KEY`), and `CODER_MODEL`; `--door gemini|glm` picks a
lane on the same door. `TYPESAFE_API_KEY` is the decision door that
`decision` graders use and that the child classifies its turns through.
`--coder PATH` names the agent binary and `--questions DIR` its question
sets; both arms get the same ones, pinned in the report's run locks.

Results go to `<eval dir>/results/TIMESTAMP/`: `report.json`,
`report.html`, `artifacts/`, `runs/CASE/ARM-N/` (the ATIF trajectory, the
created files, and the child's output, with tokens redacted), `suite/`
(the case files, byte for byte), and `run.json`. Exit codes: 0 Better or
a clean single-arm run, 1 Worse, inconclusive, or a load failure, 2
partial, 64 invalid usage, and 130 or 143 on `SIGINT` or `SIGTERM`, which
stop every live child.

`publish` uploads the suite's files to the relay's Blossom server
(`--blossom URL` to name another), publishes the suite once as a NIP-EXT
`3184` release signed by its author, and publishes the result once as a
NIP-EVAL `3189` with the report inline, signed by the evaluator; a second
`publish` reuses both. `check EVENT [TARGET]` fetches that result and its
suite, verifies every byte, refuses unless TARGET is the same plugin
(its package record and run lock), reruns the suite, and publishes a
`3189` citing the original. It prints `confirm` or `dispute` and exits 0
only on a confirm.

## Running a plugin once (`plugin run`)

`plugin purchase` buys the supported signed release through the selected
`customer` origin. Use `purchase quote --root DIR --purchase ID --plugin PUBKEY:SLUG
--input FILE --wallet-home DIR --max-msat N --max-fee-msat F` to verify the actual
packet and disclose only that supplied private text for an unpaid invoice.
Review the emitted customer, immutable release, packet/input digests, separate
endpoint and author fee, total, invoice, resident payer, and expiry. Approve its
`approval_digest` with `purchase approve --root DIR --purchase ID --digest DIGEST`,
then use `purchase invoke --root DIR --purchase ID` once. Both steps recheck
current account rights, signed registry state, supported packet, and price.

Only the explicitly selected existing resident Lightning node pays; its private
configuration determines the admitted network. Upgrade the admitted resident
to support payment with an expected node identity before purchase; the resident
checks the approved node immediately before dispatch. Changed terms require a new
purchase and approval. `purchase cancel` stops an unstarted payment, and
`purchase show` reads the retained result, execution receipt, settlement, and
charge. A receipt binding check preserves the guest's verification class;
`not_run` remains `not_run`. Unknown payment or delivery retains the original
liability and refuses another attempt or purchase for that payer until recovery
resolves it. A failed delivery can still carry a confirmed charge.

Use `plugin purchase recover --root DIR --purchase ID` after a lost payment or
delivery acknowledgment. It checks the original resident payment and reads the
provider's private retained outcome using the original purchase authorization.
Recovery does not pay or run the plugin again. Missing payment details or an
interrupted invocation remain unknown; keep the purchase reference for support.
Older purchases without recovery authorization need manual reconciliation.

```sh
openagents plugin run crates/plugin-explain-error --in ~/code/shop --request-file failure.txt
openagents plugin run crates/plugin-dependency-check --in ~/code/shop
openagents --json plugin run crates/plugin-release-notes --request "$(git log --oneline v1.4.0..v1.5.0)"
```

`plugin run` (also `ext run`) runs a plugin's workflow once on a workspace
(default: the current directory) through Coder's program runtime, the step
a Coder turn takes once it selects the workflow, and prints the reply. The
request is what you would say to Coder; a workflow that takes it
(`"request"` in its `module` binding) hands it to its Wasm, and one with
`"read_named": true` lets the Wasm read the workspace files the request
names. The grant is the plugin's own workflow with `reads` only: nothing
writes, delegates, spawns a process, or uses the network, and a workflow
that needs more is refused before its first step. The read-only smokes of
the [example plugins](../plugins/examples/README.md) and the release gate's
`explain-error` scenario use it.

## MCP server and shell completions

```sh
openagents mcp serve [--timeout SECONDS]
openagents completions bash|zsh|fish
```

`mcp serve` speaks MCP over stdio (newline-delimited JSON-RPC 2.0:
`initialize`, `notifications/initialized`, `tools/list`, `tools/call`). Its
tool list is generated from the table `openagents --help` prints: one tool
per command group except `host` (a resident server) and `mcp` itself. A
tool takes `{"args": [...]}`, the words that would follow the group on the
command line, runs `openagents --json GROUP ARGS...` in a child process with
stdin closed, and returns `{exit_code, document | stdout, stderr}` as the
structured result; a nonzero exit is an error result that still carries the
document. Pass `["--help"]` to read a group's syntax. `--timeout` bounds one
call (default 120 s). To register it with Claude Code:

```sh
claude mcp add openagents -- openagents mcp serve
```

`completions SHELL` prints a completion script for the same groups and
`--json`; source it (bash), place it on `$fpath` as `_openagents` (zsh), or
save it under `~/.config/fish/completions/openagents.fish` (fish).

## Referral sources

`openagents customer referral --help` lists account-scoped introduction commands.
Select an authenticated customer first, then use `create --input FILE` with a
private `{ "kind": "person", "label": "Your private label" }` document and
`link --referrer ID` to issue a source URL. Link issuance rotates earlier links.
`capture --input FILE` accepts `{ "request": "YOUR_RANDOM_REQUEST_ID", "token":
"rfr_...", "consent": true, "consent_version":
"openagents.referral.consent.v1" }`. The customer must explicitly agree to record
the introduction privately. Omit the token for missing attribution or set
`consent` to false for refusal. `source` reads only the selected account's source.
Existing capture cannot be overwritten; `migrate` and `accept` transfer referrer
management through two authenticated accounts, preserving the stable identity.
`disable` retires links. These commands create no commission or payment right.

`policy` reads current attribution terms; `policy --digest DIGEST` reads a retained
version. `propose --input FILE` records separate consent to that digest using
`request`, `policy_digest`, `introduction`, `referrer`, `evidence`, `reason`,
`consent`, and `expected_decision`. Evidence contains opaque private
`{ "reference": "YOUR_REFERENCE", "digest": "sha256:..." }` pairs. The introduction
is `captured_source`, `early_agreement`, `preexisting_customer`, `missing_evidence`,
or `correction`. A correction pins the current decision digest and retains the
earlier decision. `confirm --input FILE` lets the current referrer manager
explicitly confirm `{ "customer": "ACCOUNT", "decision": "sha256:..." }`.
Early agreements and corrections require both parties. Missing, competing,
self-referral, and source-only evidence remains in review; legacy sources with
unknown signup provenance are not relabeled as existing customers.

`attribution` reads the selected customer's private history. `workspace
--workspace ID` reads one relationship as its current owner or admin. New teams
inherit the original customer relationship; ownership transfer preserves it.
An existing team owner can use `adopt --workspace ID --input FILE` with
`{ "decision": "sha256:..." }` to attach their accepted relationship. Adoption
cannot overwrite another customer's binding. `lineage --referrer ID` reads
management successors. These records create no commission, payout, or spend
authority. Publish exact agreed terms through the local `tenant-referrals`
operator command before requesting consent, and qualify the intended deployment
as recorded in [NEEDS_OWNER.md](../../NEEDS_OWNER.md#referral-source-and-attribution-activation-rev-27rev-28-1083410835).

`terms [--digest DIGEST]` reads a separately published commission contract.
`accept-terms --input FILE` requires explicit `{ "request": "YOUR_REQUEST",
"customer": "ACCOUNT", "terms_digest": "sha256:...", "attribution_decision":
"sha256:...", "consent": true }`. The customer and current manager of the stable
referrer accept the same immutable terms and accepted attribution decision.
`commission --customer ACCOUNT [--agreement DIGEST]` reads a private agreement
as one of those native parties. The exact agreement digest reproduces historical
consent; omitting it reads the active mutually accepted version. A new publication
does not replace accepted terms. Current credential revocation, manager changes,
and attribution review are rechecked under the canonical account writer.

The local operator uses `tenant-referrals commission-check --input FILE` to
validate a proposed contract and derive its digest without publication. After
review, `commission-publish --registry DIR --input FILE --approve DIGEST
--expected DIGEST|none` publishes the exact sealed terms against the expected
current head. `commission-show --registry DIR [--digest DIGEST]` inspects it.
Use an owned registry directory with mode `0700` and a bounded input file with
mode `0600`.
There are no supplied commercial share or minimum defaults, and no FX conversion.
The selected existing Spark and Lightning rails accept sats, msats, or BTC
millionths, with an exactly payable whole-satoshi minimum within the native
ledger's integer bound. The contract declares whole-satoshi payout precision
and retention of unpaid msat remainders. Foreign currency terms are refused;
same-unit arithmetic does not qualify a destination or a future payout.
Publication and consent grant no accrual or payout authority. Qualify commercial
terms and the later settlement adapter before promising earnings, as recorded in
[NEEDS_OWNER.md](../../NEEDS_OWNER.md#referral-commission-contract-activation-rev-29-10836).

## Verify

```sh
cargo fmt -p openagents-cli -p openagents-wallet
cargo clippy -p openagents-cli -p openagents-wallet --all-targets -- -D warnings
cargo test -p openagents-cli -p openagents-wallet
cargo test -p openagents-wallet --test testnet -- --ignored   # reaches public testnet Esplora
```

## Playtest triage

`openagents playtest` is the triage inbox for playtest reports: `inbox`
reads the NIP-17 reports sealed to the triage key and drafts an issue for
each, `file` creates the `playtest` issue only with `--approve` (or records
one a person filed with `--issue`), `decide`, `verify`, and `session`
record the other outcomes, and `log --acceptances` lists the accepted
contributions that back playtest awards. `keygen --out PATH` creates the
triage key file (`0600`) and prints only its npub. `testflight` reads the
app's TestFlight screenshot and crash feedback from App Store Connect with an
App Store Connect API key and drafts each new submission the same way.
[docs/game/playtest-triage.md](../game/playtest-triage.md) covers the loop,
the files, and the triage log.
