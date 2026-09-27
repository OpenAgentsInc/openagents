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

## Keys and relays

```sh
openagents key list
openagents relay req '{"kinds":[33301],"#w":["verse-plaza"],"limit":20}'
openagents relay tail '{"kinds":[9],"#w":["verse-plaza"]}' --wait 60
openagents relay sign 1 "hello" --tag t=test
openagents relay publish event.json      # a file, inline JSON, or - for stdin
```

`relay` answers NIP-42 challenges with the `--as` profile key.

## Verify

```sh
cargo fmt -p openagents-cli
cargo clippy -p openagents-cli --all-targets -- -D warnings
cargo test -p openagents-cli
```
