# openagents CLI

The OpenAgents command line, in Coax. The plan for v1.0.0 is
[docs/cli/2026-11-01-openagents-cli-v1.md](../../docs/cli/2026-11-01-openagents-cli-v1.md).
Today it has two domains: `nostr`, your NIP-06 identity, and `pylon`, which
starts and stops this computer's Pylon.

## Build

```sh
coax/build.sh                    # the compiler, once
apps/openagents-cli/build.sh     # about 10 seconds; writes apps/openagents-cli/build/openagents
ln -sf "$PWD/apps/openagents-cli/build/openagents" ~/.local/bin/openagents
```

macOS and Linux. The build needs only the Coax compiler and the system C
compiler.

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

The Pylon has its own key, separate from your `nostr` identity.

## Output and exit codes

Text is `key value` lines. `--json` prints one object with `"ok"`. Exit codes:
0 done, 1 failed, 2 usage, 3 no identity, 6 not found.

## Files

| File | What |
| --- | --- |
| `main.coax` | arguments, the report, the `nostr` and `pylon` domains |
| `rules.coax` | `#authority`: profile names, the stored key's format, words as typed, hex, exit codes, the Pylon log's facts |
| `cli-native.c` | effects: Keychain, 0600 files, stdin without echo, the clock, process liveness, the executable's path |
| `build.sh` | compiles `main.coax` to C and links it with the host |

## Checks

```sh
coax/bin/coax test apps/openagents-cli/rules.coax --library
coax/bin/coax mutate apps/openagents-cli/rules.coax --library    # 295 mutants, 0 survivors (2026-11-01)
```
