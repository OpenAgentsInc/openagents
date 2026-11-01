# openagents CLI

The OpenAgents command line, in Coax. The plan for v1.0.0 is
[docs/cli/2026-11-01-openagents-cli-v1.md](../../docs/cli/2026-11-01-openagents-cli-v1.md).
Today it has three domains: `nostr`, your NIP-06 identity and a relay client;
`pylon`, which starts and stops this computer's Pylon; and `project`, the
project queue at openagents.com/projects/SLUG.

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
openagents nostr show                        # npub, public key, where the key is kept
openagents nostr sign --content "hi" [--kind 1] [--tags '[["t","coax"]]']
openagents nostr auth https://openagents.com/api/v1/systemone --body request.json
openagents nostr forget --yes
```

- The key is kept in `~/.openagents/cli/keys/PROFILE`, mode 0600, in a
  0700 directory, on every system.
- `OPENAGENTS_KEYCHAIN=1` uses the macOS Keychain instead (service
  `com.openagents.cli`, account `nostr:PROFILE`). It is off by default:
  macOS ties a Keychain item to the binary that made it, so each rebuilt
  `openagents` asks for your login password to read it. Agents rebuild and
  run the CLI all the time, so never set it for them.
- `--profile NAME` (or `OPENAGENTS_PROFILE`) keeps several identities.
- `OPENAGENTS_NSEC=nsec1...` uses a key for one run and stores nothing.
- `nostr auth` prints a NIP-98 `Authorization` header for one request:
  the URL, the method, and the SHA-256 of the body file. Sign a new one for
  each request.

```sh
curl -H "Authorization: $(openagents nostr auth https://openagents.com/api/v1/systemone --body d.json)" \
     -H 'Content-Type: application/json' --data-binary @d.json https://openagents.com/api/v1/systemone
```

## Account

```sh
openagents login                             # a link and a QR code; press APPROVE where you're logged in
openagents login --words < words.txt         # or --key-stdin: an nsec/hex secret on stdin
openagents whoami                            # the account and its keys, as the server sees this key
openagents account keys                      # the same keys, this computer's marked
openagents access                            # may this account make realms (invites)
openagents access redeem CODE                # the 32 characters after #code= in an invite link
openagents access request "what I'd make" [--contact C]
openagents logout                            # unlink this key, then delete it here
openagents --origin https://staging.openagents.com login   # a profile keeps the origin it logged in to
```

- Every call is NIP-98 signed with the profile's key
  (`coax/lib/nostr/nip98_sign.coax`); the server takes the signature as its
  key's account. The key never leaves the 0600 file except as signatures.
- `login` makes a key for the profile when it has none.
- `logout` keeps a key that is its own account (nothing linked it) unless
  you add `--yes`.

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

## Projects

```sh
openagents project list [--project SLUG]       # the queue, in the page's order
openagents project show SLUG                   # body, checks, claim, plan, notes
openagents project claim SLUG --by NAME
openagents project plan SLUG set "step" "step" # add, start N, check N, uncheck N, drop N, file PATH|-
openagents project note SLUG "text"
openagents project done SLUG --commit HASH     # a full hash already on origin/main
openagents project release SLUG
openagents project status SLUG open|resolved|closed|draft
openagents project move SLUG --before SLUG     # or --after SLUG; respread [--seed | --order FILE]
openagents project check                       # the work files' rules, no relay
openagents project publish [--dry-run]         # the work files, as events
openagents project review COMMIT --key FILE    # an independent review, coax/reviews/COMMIT.json
openagents project snapshot [--if-older MIN]   # the page's work.coax, index.md and index.html
openagents project index                       # projects/list.coax and projects/index.md
openagents project bake [--list]               # baked.coax and each commits.coax
openagents project landing [--out FILE]        # the homepage's warm start: newland-data.coax, targets, seed
openagents project publish-data [SLUG...] [--dry-run]   # a project's Markdown on the site, no release
openagents project adopt SLUG [--api URL]      # give the site a hand-made project's key
openagents project capture [--tables]          # the relay's frames for the page, as test fixtures
openagents project plan-rules                  # the Coax work seeds and the coax-nostr plan's rules
openagents project review report [--from BASE --to REF] [--notes]   # commits that weaken a check, and their reviews
openagents project review keygen FILE          # a reviewer key (mode 600); prints only its public key
```

How to work the queue is [coax/web/project/QUEUE.md](../../coax/web/project/QUEUE.md);
the events are [docs/protocol/2026-10-28-projects.md](../../docs/protocol/2026-10-28-projects.md).

- The project is `apps/openagents-web/projects/SLUG/` in the checkout the
  command runs in (`--project`, default `coax`; `--dir DIR` for another
  folder). Reading needs no key. Writing signs with the project key,
  `~/work/.secrets/openagents-project-SLUG.key` or `--key FILE`, whose
  public key must be `project.json`'s.
- A read pages through the relay (200 events a page, retried with
  reconnects) and folds the events with
  [project-fold.coax](../openagents-web/project-fold.coax), the page's own
  fold, so `list` shows what the page shows. It keeps the ids it has seen in
  `~/.openagents/cli/projects/SLUG.ids` and reads again when the relay loses
  one (`--forget` drops the file).
- `done` refuses a short hash and a commit that is not an ancestor of
  `origin/main`.
- `review` signs with the reviewer's own key and refuses an author key
  (`coax/reviews/authors.json`). It writes the file and publishes nothing.
- `snapshot`, `index` and `bake` write what the site's build bakes into the
  page (`apps/openagents-web/build.sh`). They read public events only and
  never load a key. They write a file only when its bytes change, ignoring
  the snapshot time.

## Realms

```sh
openagents realm list                              # your realms: slug|rev|published|listed|name|visibility
openagents realm show SLUG                         # its spec lines and its page
openagents realm new "a koi garden at dusk"        # plan it from words and make it, private
openagents realm new --from PLAN                   # make a plan you have
openagents realm visibility SLUG private|unlisted|public
openagents realm publish SLUG [--rev N]            # visitors see this revision
openagents realm fork SLUG [--name NAME]           # a copy you own, private
```

- Each request is signed with NIP-98 by the profile's key, so the key's
  account is the caller (`--profile NAME`, or `OPENAGENTS_NSEC`). A key no
  account linked is its own account, `n-<pubkey>`.
- A realm is `private`, `unlisted` or `public`. A new realm and a new fork
  start private: only its owner and site admins see it, and everyone else
  gets 404. Unlisted opens by its link. Public is also on `/realms` and
  `/og`. While realm making is invite only, only approved creators and site
  admins change it.
- A private realm opens at `/realms/built#SLUG`; unlisted and public realms
  have a page at `/realms/SLUG`.
- There is no delete. To take a realm down, make it private.
- The server is the one the profile logged in to, or `--origin URL`, such as
  `https://staging.openagents.com` or `http://127.0.0.1:PORT`.

## Admin

```sh
openagents admin access                            # flags and counts
openagents admin flag realm_invites on|off
openagents admin invite new [--uses N] [--days N] [--note T]   # the code and link, shown once
openagents admin invite list | revoke ID
openagents admin requests | approve ID | decline ID
openagents admin creators | creator add WHO | creator remove WHO | grandfather
openagents admin realms [--visibility V]           # every made realm and who sees it
openagents admin realm visibility SLUG V           # any realm's
openagents admin keys                              # the admin keys, as npubs
openagents admin key add NPUB [--note T] | key remove NPUB   # a root admin only
```

- `admin` works only with a site admin's key. A root admin is a key in
  `coax/web/api/admins.conf`; only a deploy changes that list. A root admin
  can add and remove database admins, and every change is audited. A
  database admin can do everything else an admin does, but cannot change the
  admin list, and no request can remove a root admin.
- `WHO` is an npub, a 64-hex public key or an account ID such as `n-HEX`.
- An invite code is printed once, and only its hash is kept. Send the link
  to the person; don't post it.

## Output and exit codes

Text is `key value` lines. `--json` prints one object with `"ok"`. Exit codes:
0 done, 1 failed, 2 usage, 3 no identity or key, 4 forbidden, 5 conflict
(changed since), 6 not found, 7 slow down.

## Files

| File | What |
| --- | --- |
| `main.coax` | the dispatch: one line per domain |
| `out.coax` | the report, arguments, numbers as text, where things live |
| `nostr.coax` | the `nostr` domain: identity, signing, NIP-98, publish and req |
| `relay.coax` | the relay client: connect, NIP-42 AUTH, one REQ to EOSE, publish and OK |
| `pylon.coax` | the `pylon` domain: start, stop, status, logs; claim, unclaim and list over the relay ([xp_keys.coax](../../coax/lib/nostr/xp_keys.coax) and [beacon.coax](../../psionic/lib/pylon/beacon.coax) decide) |
| `account.coax` | login, logout, whoami, account keys, access: signed HTTPS calls through `coax/lib/net/http.coax`; the hidden `http METHOD URL` check |
| `account_rules.coax` | `#authority`: origins, machine names, a device login's poll states, logout's plan, JSON strings, the terminal QR text |
| `project.coax` | the `project` domain: the config, the key, reading and folding the relay, `list` and `show` |
| `project-write.coax` | claim, release, note, plan, done, status, move and respread, signed with the project key |
| `project-publish.coax` | `check` and `publish`: the work files as announcements, issues, revisions, statuses and the order |
| `project-review.coax` | `review`: an independent review file signed with the reviewer's key; `review report` and `review keygen` |
| `review-rules.coax` | the review report's rules: a diff's weakened checks, a review's validity |
| `project-page.coax` | `snapshot`, `index` and `bake`: the page's tables, Markdown and lists from the relay |
| `project-landing.coax` | `landing`: the homepage's warm start, read and verified from the relay |
| `project-data.coax` | `publish-data`, `adopt` and `capture` |
| `project-plan-rules.coax` | `plan-rules`: the work seeds' and the coax-nostr plan's rules |
| `api.coax` | one signed call to the API: the origin, the NIP-98 header, the status as an exit code |
| `realm.coax` | the `realm` domain: list, show, new, visibility, publish, fork |
| `admin.coax` | the `admin` domain: invites, requests, creators, flags, realms, admin keys |
| `realm_rules.coax` | `#authority`: slugs, visibilities, flags, counts, keys and account IDs, origins, spec lines, access rows, a plan's board |
| `project-rules.coax` | `#authority`: hashes, slugs, standing, claims, plans, ranks, the order, relay paging |
| `rules.coax` | `#authority`: profile names, the stored key's format, words as typed, hex, exit codes, the Pylon log's facts |
| `cli-native.c` | effects: Keychain, 0600 files, stdin without echo, the clock, the host name, process liveness, the executable's path |
| `build.sh` | compiles `main.coax` to C and links it with the host, sockets and TLS |

A new domain goes in its own file with a `DOMAIN_main` dispatch, one
`#load` and one line in `main.coax`, and its value flags on their own line of
`VALUE_FLAGS` in `out.coax`.

## Checks

```sh
coax/bin/coax test apps/openagents-cli/rules.coax --library
coax/bin/coax mutate apps/openagents-cli/rules.coax --library    # 295 mutants, 0 survivors (2026-11-01)
coax/bin/coax test apps/openagents-cli/project-rules.coax --library
coax/bin/coax mutate apps/openagents-cli/project-rules.coax --library    # 750 mutants, 0 survivors (2026-11-01)
coax/bin/coax test apps/openagents-cli/account_rules.coax --library
coax/bin/coax mutate apps/openagents-cli/account_rules.coax --library   # 0 survivors (2026-11-01)
coax/web/api/cli-auth-test.sh https://staging.openagents.com     # the server side of login, as a Coax program
```
