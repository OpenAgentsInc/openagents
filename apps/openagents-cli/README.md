# openagents CLI

The OpenAgents command line, in Coax. The plan for v1.0.0 is
[docs/cli/2026-11-01-openagents-cli-v1.md](../../docs/cli/2026-11-01-openagents-cli-v1.md).
Today it has two domains: `nostr`, your NIP-06 identity and a relay client,
and `pylon`, which starts and stops this computer's Pylon.

## Build

```sh
coax/build.sh                    # the compiler, once
apps/openagents-cli/build.sh     # about 10 seconds; writes apps/openagents-cli/build/openagents
ln -sf "$PWD/apps/openagents-cli/build/openagents" ~/.local/bin/openagents
```

macOS and Linux. The build needs the Coax compiler and the system C
compiler; on Linux, TLS comes from OpenSSL through Nix
([coax/platform/tls-env.sh](../../coax/platform/tls-env.sh)).

## Identity

```sh
openagents nostr new                         # 12 new words (shown once) and their NIP-06 key
openagents nostr import --words < words.txt  # or type them; nothing goes in argv
openagents nostr import --nsec               # an nsec1... or 64-hex secret on stdin
openagents whoami                            # npub, public key, where the key is kept
openagents nostr sign --content "hi" [--kind 1] [--tags '[["t","coax"]]']
openagents nostr auth https://openagents.com/api/v1/systemone --body request.json
openagents nostr forget --yes
```

- The key is kept in the macOS Keychain (service `com.openagents.cli`,
  account `nostr:PROFILE`). On Linux it is
  `~/.openagents/cli/keys/PROFILE`, mode 0600, in a 0700 directory.
- `--profile NAME` (or `OPENAGENTS_PROFILE`) keeps several identities.
- `OPENAGENTS_NSEC=nsec1...` uses a key for one run and stores nothing.
- `nostr auth` prints a NIP-98 `Authorization` header for one request:
  the URL, the method, and the SHA-256 of the body file. Sign a new one for
  each request.

```sh
curl -H "Authorization: $(openagents nostr auth https://openagents.com/api/v1/systemone --body d.json)" \
     -H 'Content-Type: application/json' --data-binary @d.json https://openagents.com/api/v1/systemone
```

## Relays

```sh
openagents nostr publish --content "hi" [--kind 1] [--tags JSON] [--relay wss://relay.openagents.com]
openagents nostr req '{"ids":["ID"]}' [--relay URL] [--limit 500] [--timeout 20]
```

`publish` signs the event with your key, publishes it and waits for the
relay's OK; it prints `id`, `auth` (whether the relay asked for and accepted
NIP-42 AUTH) and `ok`. `req` sends one filter and prints each event, one JSON
line each, until EOSE, `--limit` or `--timeout`. Every event it prints has a
checked id and signature; one that fails is dropped. Both answer the relay's
NIP-42 challenge with your key when you have one. The relay is
`wss://relay.openagents.com` unless you pass `--relay`.

## Pylon

```sh
openagents pylon start --wait 120            # the toy model; --model elephant-1 for the real one
openagents pylon status                      # running, online, slug, npub
openagents pylon logs [--lines N] [--engine | --join]
openagents pylon stop
```

`pylon start` runs [scripts/join-pylon.sh](../../scripts/join-pylon.sh) in
the background in its own session, with output in
`~/.openagents/pylon/cli-join-MODEL.log`. It passes the join everything
after `--model`: `--label`, `--slug`, `--relay`, `--device`,
`--model-cache` and `--self-test`. By default it builds from the checkout
the CLI was built in (`--source`). Set `OPENAGENTS_SOURCE` to use another
checkout, or pass `--official` to let the join use its own clone of `main`.
`pylon stop` writes the join's stop file and waits for it to exit.

The Pylon has its own key, separate from your `nostr` identity. It links
that key to yours instead of sharing one (NIP-XP; docs/web-log-in.md,
"Linking a Pylon to its person"):

```sh
openagents pylon start --wait 120            # passes the join --trainer YOUR-NPUB (--no-link skips, --trainer NPUB names another)
openagents pylon claim                       # your trainer profile (13193) lists the running Pylon's key
openagents pylon unclaim [--key NPUB]        # drops it
openagents pylon list [--mine] [--relay URL] # the network's Pylons from their beacons
```

- `start`: once online, the Pylon publishes a key link (13195) naming you.
- `claim` reads your newest profile from the relay, keeps the keys it
  already lists (at most 16) and its `shown`, adds the Pylon's key (from
  this computer's Pylon log, or `--key`) and publishes the new profile. It
  reports `linked`: whether the Pylon's newest link names you, which the
  link needs both ways.
- `list` prints slug, model identity, status, free slots, age, npub and
  label, newest first. Each beacon is verified (id, signature, content
  digest, schema) and fresh by this computer's clock, as the homepage
  reads them. `--mine` keeps the Pylons whose newest link names you.

## Output and exit codes

Text is `key value` lines. `--json` prints one object with `"ok"`. Exit codes:
0 done, 1 failed, 2 usage, 3 no identity, 6 not found.

## Files

| File | What |
| --- | --- |
| `main.coax` | the dispatch: one line per domain |
| `out.coax` | the report, arguments, numbers as text, where things live |
| `nostr.coax` | the `nostr` domain: identity, signing, NIP-98, publish and req |
| `relay.coax` | the relay client: connect, NIP-42 AUTH, one REQ to EOSE, publish and OK |
| `pylon.coax` | the `pylon` domain: start, stop, status, logs; claim, unclaim and list over the relay ([xp_keys.coax](../../coax/lib/nostr/xp_keys.coax) and [beacon.coax](../../psionic/lib/pylon/beacon.coax) decide) |
| `account.coax` | HTTPS calls through `coax/lib/net/http.coax`; the hidden `http METHOD URL` check |
| `rules.coax` | `#authority`: profile names, the stored key's format, words as typed, hex, exit codes, the Pylon log's facts |
| `cli-native.c` | effects: Keychain, 0600 files, stdin without echo, the clock, process liveness, the executable's path |
| `build.sh` | compiles `main.coax` to C and links it with the host, sockets and TLS |

A new domain goes in its own file with a `DOMAIN_main` dispatch, one
`#load` and one line in `main.coax`, and its value flags on their own line of
`VALUE_FLAGS` in `out.coax`.

## Checks

```sh
coax/bin/coax test apps/openagents-cli/rules.coax --library
coax/bin/coax mutate apps/openagents-cli/rules.coax --library    # 295 mutants, 0 survivors (2026-11-01)
```
