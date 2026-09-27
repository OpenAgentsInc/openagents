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
  `host/` and `coder-access/`. No command prints a secret key, a bearer
  key, or an invitation after the moment it is created.

## Build and install

```sh
cargo build --release -p openagents-cli
install target/release/openagents ~/.local/bin/
```

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
under `~/.openagents/session/` (`OPENAGENTS_SESSION_HOME` or `--store PATH`
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
```

`--xp-relay` chooses the XP relay (default `VERSE_XP_RELAY`, then the world
relay); `--referee KEY` trusts another referee for this reading only.
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
`release`, `stop`, `wait SECONDS`, `status`, `parts`. `install` carries the
held part to its slot, correcting for the carry offset, and fails if it does
not latch. The speed, thrust, tether, and latch limits are the simulation's
own.

To drive a zone another client simulates, `openagents zone send VERB...
--to OPERATOR_PUBKEY` publishes the verbs as NIP-MV kind `23302` zone
commands, and `openagents zone listen` prints the commands addressed to
this identity. The `23302` kind is a proposed extension in
`nips/openagents/NIP-MV.md`; the Verse desktop client does not yet accept
it, so today the local simulation is the only operator.

## Sovereign agents (NIP-SOV)

```sh
openagents sov status
openagents sov profile new NAME --agent PUBKEY --authority PUBKEY --policy REF ...
openagents sov profile validate profile.json
openagents sov spawn NAME
```

`spawn` fails closed. NIP-SOV binds activation to an admitted authority,
a custody adapter, a policy store, a controller, an environment lease, and
checkpoints, and none of those exist yet; the command names exactly which
preconditions are missing rather than starting an unbounded process.

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

```sh
openagents cap list --profile executor --limit 20
openagents cap describe PUBKEY:SLUG
openagents prg list --step delegate
openagents prg describe --author PUBKEY SLUG
openagents ext list                        # listings; --type release|revocation|migration|checkpoint
openagents ext list --package ROOT_PUBKEY:SLUG
openagents discover --origin https://openagents.com
openagents discover --fetch --timeout 5    # compare what the origin serves
```

`cap`, `prg`, and `ext` read published heads from one relay (`--relay`,
`--timeout`, and `--as` for NIP-42) and never run a probe, install a
package, or mint a grant. Every record carries `valid` and, when the
signature, kind, marker, or body fails the contract, a `refusal`, so a
malformed head shows up instead of vanishing. `describe` exits 1 when the
head it found is invalid. `discover` prints the agent card and agent-skills
index this checkout serves for an origin; `--fetch` also reads both from the
origin over HTTP and exits 1 when either differs or fails to load.

## Verify

```sh
cargo fmt -p openagents-cli
cargo clippy -p openagents-cli --all-targets -- -D warnings
cargo test -p openagents-cli
```
