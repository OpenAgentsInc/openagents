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

- `--json` anywhere on the command line switches every command to one JSON
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
(`coder::task::studio`, [the specification](../verse/agent-studio.md)) on
this computer's task store. Seats bind a name and role to an auto-start
route. A goal starts a lead task whose reply ends with a plan; the
coordinator validates the plan, holds each entry until the tasks it depends
on are done, and then submits it to the inbox and notes it eligible for the
auto-start policy. An invalid plan, a lead without a plan, and a failed
dependency each become a decision on the goal.

```sh
openagents studio seat set lead --role lead --route codex:gpt-6-luna
openagents studio seat set ada --route claude:claude-opus-5-5
openagents studio goal submit "Add a --verbose flag" --workspace openagents
openagents --json studio goal list
openagents studio plan list GOAL
openagents studio message ada "Keep commits small."
```

The resident host runs the same reconciliation with each auto-start sweep;
`openagents studio sync` runs it now.

## Verse (NIP-MV)

Headless presence: see who is around, listen, speak, move, and gesture.
Every event is verified before it counts, and stale states never overwrite
newer ones.

```sh
openagents key show                     # this identity's public key (creates it)
openagents verse who --json             # every entity with a state, nearest first
openagents verse look --at 0,0,0 --radius 1 --wait 5
openagents verse move 3,0,-2 --yaw 90 --name devin
openagents verse say "hello" --to near
openagents verse gesture greet --to PUBKEY,avatar
openagents verse tail --wait 60 --json  # poses, gestures, states, chat as they arrive
openagents verse leave
```

`--relay` defaults to `wss://relay.openagents.com`, `--world` to
`verse-plaza`, and `--as` names the profile key (default `default`).

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
