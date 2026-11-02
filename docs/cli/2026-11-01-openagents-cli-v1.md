# The openagents CLI, v1.0.0

Status: spec, 2026-11-01. The first slice is built in
[apps/openagents-cli](../../apps/openagents-cli/README.md): the binary, the
`nostr` domain (a NIP-06 identity in a 0600 key file, event signing and
NIP-98 headers) and the `pylon` domain (start, stop, status and logs). The
`account` and `realm` domains below are not built yet. The routes they call
exist unless a row says **new**.

`openagents` is one native binary, written in Coax. You use it to log in to
your OpenAgents account and work with what the account owns. It has one
domain per area of the product, the same way the pre-Nuke CLI had `pylon`,
`wallet` and `chat`. Version 1.0.0 ships with two domains: `account` and
`realm`. The `realm` domain is Aethyrforge's programmatic surface. It lets
you, a script or a coding agent such as Claude Code make and edit realms
without opening the page.

```sh
openagents login
openagents realm new "a walled garden with a koi pond at dusk" --make
openagents realm move walled-garden trees:3 4.5 -2
openagents realm publish walled-garden
openagents realm shot walled-garden garden.png
```

## Goals

- **Agents first.** Every command works without a terminal: no prompts when
  stdin is not a TTY, stable text output, `--json` on every command, and
  exit codes that tell a script what went wrong.
- **One identity.** The CLI acts as your account, with a Nostr key that the
  account has linked. It never holds a password or a cookie.
- **The page and the CLI edit the same thing.** A realm edit from the CLI
  goes through the same routes, the same SQL rules and the same Coax edit
  operations (`aethyr/forge/edit.coax`) as the page. There is no second
  editing path to keep in step.
- **Coax only.** The binary is Coax on the C tool host
  (`coax/platform/tool-native.c`), per [tools in Coax](../../coax/tools-in-coax.md).
  The host does platform effects only: sockets, TLS, files, the Keychain.

## What we had before the Nuke

`crates/openagents-cli` (read it with `git show 0b401561a0^:crates/openagents-cli/src/main.rs`)
was a Rust binary with about 150 modules and 80 top-level commands in one
flat `match`: `coder`, `chat`, `pylon`, `wallet`, `pay`, `login`, `keys`,
`verse`, `zone`, `sales`, `foreman`, `cloud`, `deploy` and many more.

What we keep:

- One binary named `openagents`, with `openagents GROUP VERB`.
- A global `--json` flag that switches every command to JSON.
- `openagents help GROUP [VERB]` as the same text as `GROUP VERB --help`.
- Usage errors exit with their own code.
- Agents sign requests with NIP-98 instead of holding cookies.

What we drop:

- Groups with no live server surface behind them. A domain ships only when
  its routes exist and are tested.
- Hand-written dispatch and help text. In v1, one command table drives the
  parser, help, the MCP tool list and the reference table in this doc
  (see [One command table](#one-command-table)).
- Commands for the retired stacks (the old Coder Cloud Run service, Foreman,
  sales, cloud pools, Boat). They don't return. The new `coder` domain below
  is a different thing: a local-first coding agent built in Coax.

## Shape

```text
openagents [GLOBAL FLAGS] DOMAIN VERB [ARGS] [FLAGS]
openagents login | logout | whoami        shortcuts for `account login` and so on
```

Global flags:

| Flag | Meaning |
| --- | --- |
| `--json` | Answer in JSON, one object (or one object per line for streams). |
| `--profile NAME` | Use another saved login (default `default`). `OPENAGENTS_PROFILE` too. |
| `--origin URL` | Talk to another server, for example `https://staging.openagents.com`. Saved per profile. |
| `--yes` | Skip confirmation for commands that ask (publish, delete). Without a TTY they refuse unless `--yes` is given. |
| `--quiet` | Print only the result value (a slug, a URL, a revision). |

### Output

Text output is lines of `KEY VALUE` or rows of fields, the same shape the API
answers with `?as=lines`, so it reads well and splits with `cut` or `awk`.
`--json` returns the API's JSON, wrapped as `{"ok": true, ...}` or
`{"ok": false, "error": "...", ...}`. Errors go to stderr in text mode.

### Exit codes

| Code | Meaning | From |
| --- | --- | --- |
| 0 | done | 2xx |
| 1 | failed (network, server error, a refused edit) | 5xx, other |
| 2 | usage: bad flags or arguments | the parser |
| 3 | not logged in, or the key is not linked | 401 |
| 4 | forbidden: not your realm, or invite needed | 403 |
| 5 | changed since: the realm moved past the revision you named | 409 |
| 6 | not found | 404 |
| 7 | slow down: a limit or the builder's daily cap | 429 |

On code 5 the output includes `rev N`, the revision there now, so an agent
can read the realm again and retry.

## Accounts and keys

An account is `n-<pubkey>` today, or `w-<hash>` once WorkOS lands, with
linked identities in `auth.identities`
([realm invites and accounts](../auth/2026-10-31-realm-invites-and-auth.md)).
The CLI is one more identity: a Nostr key that it generates, keeps, and
links to your account. Every request it makes is signed with NIP-98
(kind 27235, URL, method, payload hash; `nostr/book/ch049-http-auth.md`),
which the server already checks in `nip98.coax`.

### `openagents login`

The default flow approves the CLI from a browser where you're already logged
in, the reverse of the phone QR flow at `/login`:

1. The CLI uses the profile's key, or generates a fresh one and stores it
   (below). Built in `apps/openagents-cli/account.coax`; its decisions
   (origin, machine name, poll states, logout, the QR text) are in
   `account_rules.coax`, `#authority`, every mutant killed.
2. It signs `POST /api/auth/v2/device/start` (**new**) and gets a short code
   and an ID.
3. It prints a link, `https://openagents.com/login/device#code=<32 hex>`,
   and the same link as a QR code in the terminal (`coax/lib/qr`, half
   blocks), on stderr. It never opens a browser itself.
4. On that page (**new**, drawn in Coax like `/login`), you see the code, the
   machine name the CLI sent, and APPROVE. Approving links the CLI's key to
   your account (`auth.identities`, kind `nostr`, with label `cli:<machine>`).
5. The CLI polls `GET /api/auth/v2/device/<id>` (**new**, signed) every 2
   seconds for up to 10 minutes (`--poll SECONDS` for less), then prints
   `account ACCOUNT` and exits 0. A key already linked to another account
   answers that account at once (`already true`); `--again` asks anyway.
   `--machine NAME` names the computer (default: its host name).

Other ways in, for machines with no browser:

| Command | What it does |
| --- | --- |
| `openagents login --words` | Reads your 12 words from stdin (never argv) and derives the NIP-06 key, the same account the `/login` words make. |
| `openagents login --key-stdin` | Reads an `nsec` or 64-hex secret from stdin. |
| `OPENAGENTS_NSEC=nsec1...` | Uses this key for the one process and stores nothing. For CI and sandboxes. |

A key that no account has linked is its own account (`n-<pubkey>`), as on the
web. That's enough to make and own realms without a browser at all.

### Where the key lives

| Platform | Store |
| --- | --- |
| macOS | The same 0600 file as Linux (built). The Keychain (service `com.openagents.cli`) is opt-in with `OPENAGENTS_KEYCHAIN=1`: macOS ties an item to the binary that made it, so every rebuilt CLI asked for the login password, and agents rebuild it all day (owner, 2026-11-01). |
| Linux | `~/.openagents/cli/keys/PROFILE` mode 0600 in a 0700 directory (built), as an NIP-49 `ncryptsec` when `--encrypt` is given (`coax/lib/crypto/scrypt_nip49.coax`), else the raw key with a warning at login. |

`~/.openagents/cli/origins/PROFILE` (0600) holds the origin a profile
logged in to, never a secret; `--origin` overrides it for one run. The key is never printed, logged, put in
argv or sent anywhere except as signatures. `openagents logout` asks the server to unlink the
key (`POST /api/auth/v2/unlink {pubkey}`, **new**), then deletes it. A key
that is its own account (nothing linked it) is kept unless `--yes` is
given: deleting it would lose that account.

### Account commands

| Command | Route |
| --- | --- |
| `openagents whoami` | `GET /api/auth/v2/me`: account, identities, whether you're an approved creator |
| `openagents account keys` | the identities linked to the account, this CLI's marked |
| `openagents access` | `GET /api/playtest/access`: invites on or off, whether you may make realms |
| `openagents access redeem CODE` | `POST /api/playtest/access/redeem` |
| `openagents access request "what I'd make"` | `POST /api/playtest/access/request` |

### What the server needs

The realm routes in `coax/web/api/realms.c` find the caller from the
`X-Verse-Player` header, a bearer key minted per browser, and treat a
NIP-98-signed request only as a possible site admin (`admin_who`). For the
CLI:

1. **NIP-98 as the caller.** A request that carries a valid NIP-98
   `Authorization` header is made by the key's account
   (`auth.account_of(pubkey)`), on every `/api/playtest/realms` route. The
   ownership rules in SQL (`verse.edit_role`) don't change; they get an
   account where they used to get a player key. The cross-site `Origin`
   check stays; a CLI sends no `Origin`. Each signed write binds its body
   with the payload tag and is good once, as the invariant already says.
2. **Device login**: `device/start`, `device/<id>`, the approve page and
   `unlink`, in `access.c`, with codes kept as SHA-256 only, 10 minutes to
   live, approved once, and the same limits as invite redemption.
3. **A list of your realms**: `GET /api/playtest/realms/mine` (**new**):
   slug, name, revision, published revision, listed. Today no route lists
   them.

Built (`migrations/0098_cli_callers.sql`):
`caller-policy.coax` decides a signed caller (a bad, unbound or replayed
signature is 401, never the player key); the signer acts through a player
key SQL derives for its account (`auth.caller_key`), so the realm rules are
unchanged. The shared signer is `coax/lib/nostr/nip98_sign.coax`; each event
carries a `nonce` tag, so two requests to one URL in one second are two
events. `GET /api/auth/v2/me`, `device/*` and `unlink` answer with the
`auth_v2` flag off. Device codes are 128 bits (32 hex), and
`POST /api/auth/v2/device/look {code}` gives the page the machine name.
APPROVE is a press on the page, never automatic. A key with realms, players
or creator rows of its own is refused (409), as `link/nostr` refuses it.
The checks are Coax programs: `coax/web/api/tests/cli_auth.coax`, run by
`coax/web/api/test.sh` and against staging by `coax/web/api/cli-auth-test.sh`.

Limits stay as they are and count against the account: the builder's daily
cap, one plan every 2 seconds, 12 realms a day, 200 placed things, 64 moves
and 64 undo steps a realm. The invite gate applies to CLI requests exactly
as to the page (exit 4 with `need invite`).

## Who sees a realm (built, 2026-11-01)

A made realm has a visibility, `private`, `unlisted` or `public`
(`coax/web/api/migrations/0099_realm_visibility.sql`):

| Visibility | Who sees it | `/realms/SLUG` page | On `/realms` and `/og` |
| --- | --- | --- | --- |
| `private` | its owner and site admins; 404 to everyone else | none; the owner opens `/realms/built#SLUG` | no |
| `unlisted` | anyone with the link | yes, `noindex` | no |
| `public` | everyone | yes | yes |

- A new realm and a new fork start private. At the migration, realms that
  were listed became public, unlisted realms stayed unlisted, and the
  built-in realms stayed public.
- Private applies to every route under the realm's slug: the spec, `/doc`,
  the panes, the card, a fork by anyone else, and every edit. The rule is
  `verse.realm_hidden`; `realms.c` asks it once, before any slug route.
- `POST /api/playtest/realms/<slug>/visibility {visibility}` changes it.
  Its owner may when the owner is an approved creator (or realm making isn't
  invite only), and a site admin may for any realm. The rule is
  `verse.realm_visibility_may`. Making a realm unlisted or public writes its
  page; making it private removes the page, its text, its card and its line
  in `realms/made-slugs.txt`.
- `publish`'s `listed` (and a built-in fork's `published`) still works and
  maps onto visibility: listed is public, unlisted is unlisted. It needs the
  same right.
- `GET /api/playtest/realms/mine` adds the visibility as the last field of
  each line and in JSON.
- There is no delete route. Making a realm private takes it off the site.

CLI: `realm visibility SLUG V`, and `admin realm visibility SLUG V` for any
realm. `admin realms [--visibility V]` lists every made realm
(`GET /api/admin/access/realms`, lines `realm SLUG|VISIBILITY|REV|PUBLISHED|OWNER|NAME`).

## The admin domain (built, 2026-11-01)

Every `admin` verb is one route in `coax/web/api/access.c`, signed with
NIP-98 by a site admin's key:

| Command | Route |
| --- | --- |
| `admin access` | `GET /api/admin/access` (the `F`, `I`, `R`, `C` rows, summed) |
| `admin flag NAME on\|off` | `POST /api/admin/access/flag {name, on}` |
| `admin invite new [--uses N] [--days N] [--note T]` | `POST /api/admin/access/invites` |
| `admin invite list` / `revoke ID` | the `I` rows / `POST .../invites/ID/revoke` |
| `admin requests` / `approve ID` / `decline ID` | the `R` rows / `POST .../requests/ID/approve\|decline` |
| `admin creators` / `creator add WHO` / `creator remove WHO` | the `C` rows / `POST .../creators {who}` / `POST .../creators/ACCOUNT/remove` |
| `admin grandfather` | `POST /api/admin/access/grandfather` |
| `admin realms [--visibility V]` | `GET /api/admin/access/realms` |
| `admin keys` | `GET /api/admin/access/admins` |
| `admin key add NPUB [--note T]` / `key remove NPUB` | `POST .../admins {pubkey, note}` / `POST .../admins/HEX/remove` |

For everyone: `access` (`GET /api/playtest/access`), `access redeem CODE`
and `access request TEXT [--contact EMAIL]`.

### Admin keys

There are two kinds of site admin
(`coax/web/api/migrations/0100_admin_keys.sql`):

- A **root admin** is a public key in `coax/web/api/admins.conf`
  (`OA_ADMIN_PUBKEYS`). Only a deploy adds or removes one.
- A **database admin** is a row in `access.admin_keys`, added by a root
  admin.

Both pass every admin check (`admin.c`, `is_admin`). Only a root admin adds
or removes a database admin (`access.admin_may`). A database admin cannot
change the list, no request can remove a root admin (409), and every add and
remove is a row in `access.admin_audit`.

## The realm domain (Aethyrforge)

Every verb maps to one route under `/api/playtest/realms`. What each route
does is in [realm-builder.md](../verse/realm-builder.md) and
[realm-edit.md](../verse/realm-edit.md); the concepts are in the Aethyr book,
chapters 51 to 55 and 82.

### Make a realm

| Command | Route | Notes |
| --- | --- | --- |
| `realm new "WORDS" [--setup K=V,...]` | `POST /plan?as=lines` | Prints the board: plan ID, then one row a line, `N on\|off FEATURE LABEL`, then any `want THING` lines. `--setup` takes `camera=third\|first\|top\|side`, `style=lowpoly\|painterly\|flat`, `ground=...`, `time=...`; omitted fields are the model's pick. |
| `realm new --doc FILE` | `POST /plan/doc?as=lines` | A design document (.md, .txt, .pdf, .docx, 360 KB at most). |
| `realm revise-plan PLAN "WORDS"` | `POST /plan` with the plan and switches | Revise the board before making it. |
| `realm make PLAN [--on 0,2,5 \| --all]` | `POST /realms?as=lines` | Makes exactly the board's checked rows. Prints `slug`, `page`. |
| `realm new "WORDS" --make` | both | Plans and makes every row the model checked, in one step. |
| `realm fork SLUG [--name NAME]` | `POST /<slug>/fork?as=lines` | |

### Read a realm

| Command | Route |
| --- | --- |
| `realm list` | `GET /mine` (**new**) |
| `realm show SLUG` | `GET /<slug>`: the spec lines (role, rows, tools, placed things, revision, published, waiting) |
| `realm doc SLUG` | `GET /<slug>/doc`: the document (`aethyr.realm` v1) and its SHA-256 |
| `realm asks SLUG` | `GET /<slug>/asks` |
| `realm proposals SLUG` | `GET /<slug>/proposals` |
| `realm panes SLUG` | `GET /<slug>/panes`: lore and quests |
| `realm open SLUG` | prints `https://openagents.com/realms/<slug>` (opens it with `--browser`) |
| `realm shot SLUG OUT.png [--phone]` | the page in headless Chrome (`coax/web/shot.coax`, muted, never a visible window), 1440x900 or 390x844, as the owner when the profile owns it |

`realm shot` is how an agent sees what it did. It's the one command that needs
Chrome on the machine.

### Edit a realm

Every edit names the revision it was read at. Without `--rev`, the CLI reads
the realm first and uses its current revision. With `--rev N`, a realm that
has moved past N is refused with exit 5, so an agent never overwrites a
change it didn't see.

| Command | Route |
| --- | --- |
| `realm switch SLUG ROW on\|off` | `POST /<slug>/switch {row, on}` |
| `realm revise SLUG "WORDS"` | `POST /<slug>/revise {prompt}`: the model changes the board under the realm's schema |
| `realm ask SLUG "WORDS"` | `POST /<slug>/asks {words}`: adds to the feature list, applied or waiting for code |
| `realm ask SLUG --doc FILE` | `POST /<slug>/asks/doc` |
| `realm ask-delete SLUG ID` | `POST /<slug>/asks/<id>/delete` |
| `realm ask-done SLUG ID --commit HASH` | `POST /<slug>/asks/<id>/done`: site admins, after the code is pushed ([verse/SKILL.md](../../verse/SKILL.md), "Asks waiting on code") |
| `realm row-delete SLUG ROW` | `POST /<slug>/rows/<key>/delete` |
| `realm move SLUG ENTITY X Z` | `POST /<slug>/save {rev, moved}`: the document's moves with this one applied by `forge_apply`. `ENTITY` is `feature:index`, such as `trees:3`; X and Z in metres, within 40 m of the middle |
| `realm place SLUG THING X Z [--yaw DEG]` | `POST /<slug>/place` |
| `realm settings SLUG --tools build,talk,...` | `POST /<slug>/settings {tools}` |
| `realm lore SLUG WHO "TEXT"` | `POST /<slug>/lore` (400 characters; `""` forgets it) |
| `realm quest SLUG GIVER --ask A --step S1 --step S2 --reward R` | `POST /<slug>/quest` |
| `realm accept SLUG ID`, `realm decline SLUG ID` | `POST /<slug>/proposals/<id>/accept\|decline` |
| `realm undo SLUG` | `POST /<slug>/undo {rev}`: puts back the revision before the newest step |
| `realm publish SLUG [--listed]` | `POST /<slug>/publish {rev, listed}`: visitors see this revision. Asks for confirmation on a TTY |

Words (`revise`, `ask`, `new`) go to the server's planner, which answers under
a schema of the realm's rows. The CLI matches no words itself. It parses only
bounded fields: slugs, entity keys, numbers, enums and IDs.

### Batch edits

`realm apply SLUG FILE [--rev N]` reads JSON lines, one operation a line, and
runs them in order:

```json
{"op":"move","entity":"trees:3","x":4.5,"z":-2}
{"op":"move","entity":"rocks:0","x":-6,"z":8}
{"op":"switch","row":"lanterns","on":true}
{"op":"place","thing":"barrel","x":2,"z":3,"yaw":90}
```

Consecutive moves become one save, so one undo step. Each other operation is
its own request and its own revision. The first refusal stops the batch and
prints how many lines were applied and the revision now. In v1 a batch is
not atomic; a server-side `POST /<slug>/apply {rev, ops}` that is (one
revision, one undo step) comes with chapter 52's "Still to do": every edit
as an operation in one log.

## Connecting Claude Code and other agents

Three ways, all on the same binary and the same login:

1. **Run it.** An agent with a shell runs `openagents realm ...` and reads the
   output. Nothing to configure beyond `openagents login`.
2. **The skill.** `openagents skill realm` prints a `SKILL.md` for realm
   editing: what a realm is, the edit loop (show, edit with `--rev`, shot,
   publish), the exit codes, and the limits. `openagents skill realm --install`
   writes it to `.claude/skills/openagents-realm/SKILL.md` in the current
   directory. The text comes from the command table, so it can't drift.
   [verse/SKILL.md](../../verse/SKILL.md) stays the skill for building a
   hand-made realm in code and opening a pull request.
3. **MCP.** `openagents mcp` serves the same verbs as MCP tools over stdio:

   ```sh
   claude mcp add openagents -- openagents mcp
   ```

   Each tool is one command-table row (`realm_show`, `realm_move`,
   `realm_publish`, ...) with its arguments as a JSON schema. Tool results
   are the `--json` output. `realm_shot` returns the PNG as an image.
   `--domains realm` limits the tools offered; `--read-only` offers only
   reads.

The loop an agent runs:

```text
realm show SLUG            → rev 7, the rows, what's placed
realm move SLUG ... --rev 7  → rev 8   (or exit 5: read again)
realm shot SLUG out.png    → look at it
realm publish SLUG --yes   → published 8
```

## One command table

Each command is one row in `apps/openagents-cli/commands.coax`: domain,
verb, arguments (name, kind, bounds), flags, method, route, and one line of
help. From that table:

- the parser checks arguments before any request is made;
- `--help` and `openagents help` print it;
- `openagents mcp` lists its tools and their schemas;
- `openagents skill` writes its skills;
- `openagents commands --markdown` prints the reference tables in this doc.

A `#test` checks every row: the route is one `realms.c` (or `access.c`)
answers, every argument has bounds, and no two rows share a verb.

## Where it lives and how it builds

```text
apps/openagents-cli/
  main.coax          tool_main: global flags, profile, dispatch by the table
  commands.coax      the command table
  account.coax       login, logout, whoami, keys, access
  realm.coax         the realm domain
  mcp.coax           MCP over stdio, from the table
  skill.coax         skills, from the table
  build.sh           a thin wrapper: coax0 + the C host into build/openagents
  README.md
```

What it needs from `coax/lib`, in order:

| Library | Status |
| --- | --- |
| `sys/args.coax`, `sys/fs.coax`, `sys/proc.coax`, `json/json.coax` | exist |
| `net/http.coax`: an HTTP/1.1 client over `socket-native.c` and `tls-native.c` (GET/POST, headers, chunked bodies, redirects, timeouts) | **to build first**; `tools-in-coax.md` lists it as blocking other ports too |
| NIP-98 signing for any URL, method and body | exists twice: `coax/lib/nostr/weblogin.coax` (words key, POST) and `coax/lib/gateway/ours_sign.coax`; lift one shared signer into `coax/lib/nostr/` |
| `platform/keychain-native.c` with a `#foreign` wrapper | to build (macOS) |
| `qr` in the terminal (half-block characters) | `coax/lib/qr` exists; add a text renderer |

The key store and the signer decide who acts as an account, so their file is
`#authority` and gets `coax mutate` with zero survivors. The parser and table
checks are `#test`s that run in seconds.

Builds: macOS arm64 first, Linux x86_64 next. `openagents version` prints the
CLI version, the `coax0` version that built it, and the protocol version.

Install, once there's a release: `curl -fsSL https://openagents.com/cli/install.sh | sh`
(**new**: downloads the binary for the machine and checks its SHA-256 against
a signed release note). Until then, from a clone:

```sh
coax/build.sh && apps/openagents-cli/build.sh && ln -sf "$PWD/apps/openagents-cli/build/openagents" ~/.local/bin/openagents
```

The old Rust CLI used the same name. Version 1.0.0 replaces it; on first run
the new binary ignores the old `~/.openagents/` directory and leaves it alone.

## Domains after v1.0

Each one ships when its server surface exists:

| Domain | What | Waits on |
| --- | --- | --- |
| `wallet` | balance, receive, send, Lightning address (Spark) | [the Spark wallet](../lightning/README.md) |
| `maker` | claim a realm, set a Lightning address, list upgrades, see tips | [makers](../verse/makers.md) routes taking NIP-98 |
| `project` | the `/projects` queue as signed Nostr events: list, claim, plan, done | the port of `coax/web/project/*.ts` |
| `nostr` | sign, publish, query relays | `coax/lib/nostr` client, relay pool |
| `pylon` | link a Pylon to the account, see its jobs and XP | [connect a Pylon](../connect-a-pylon.md) |
| `realm watch` | follow a realm's revisions and proposals live | the room service |

## The coder domain

Added 2026-11-01. `openagents coder` runs Coder, OpenAgents' coding agent, as
specified chapter by chapter in [Coder: Coding Agents From Scratch](../../coder/book/README.md).
OpenAgents is the general agent that routes and replies in every channel;
Coder is the Worker that takes one repository task to a pull request
([the split](../../coder/book/PLAN.md#openagents-and-coder)).

Shipped 2026-11-02: `run` (with `--detach`), `ls`, `status`, `log`, `stop`,
`fork` and `route`
(`apps/openagents-cli/coder.coax` runs `coder/tui/coder.sh`, which owns the
verbs). Sessions are JSON-lines files in `~/.openagents/coder/sessions`. `run`
exits 0 when the task finished, 1 when it failed and 130 when it was stopped;
`status`, `log`, `stop` and `fork` exit 6 for an unknown ID. Without
`--engine`, `run` asks System One which engine to use (`route` prints only
that decision). `--on` isn't there yet. The other verbs are still planned.

| Verb | What it does | Needs a server |
| --- | --- | --- |
| `coder run "TASK" [--engine micro\|claude\|codex\|psionic] [--on mac\|cloud] [--cwd DIR] [--detach]` | start a task; prints its ID and streams its steps | only for `--on cloud` |
| `coder ls` | tasks with state, engine, Thread and age | no |
| `coder status ID` | one task's state, last step and ending | no |
| `coder log ID` | the task's session log, as text or `--json` lines | no |
| `coder attach ID` | follow a running task and send it messages | no |
| `coder stop ID` | stop a task and its tools | no |
| `coder fork ID [--at TURN]` | a new task from a turn boundary | no |
| `coder replay ID` | rebuild every request from the log and compare bytes | no |
| `coder watch THREAD` | subscribe Coder to a Thread such as `github:owner/repo#123` | yes |
| `coder policy list\|add\|rm` | trigger policies for acting without being asked | yes |
| `coder daemon` | attach this machine as an execution environment | yes |
| `coder bench SUITE` | run a harness suite and report it | no |

This domain is an exception to "a domain ships only when its routes exist":
its local verbs need no routes. The verbs marked "yes" wait for the routes in
the book's chapters 17 and 20, and ship under the normal rule. Without
`--engine`, `run` asks a System One decision which engine to use (chapter 13).
Every verb is in the one command table, so each is also an MCP tool.

## Order of work

1. `coax/lib/net/http.coax` with `#test`s against a local server.
2. The shared NIP-98 signer, and the server accepting NIP-98 as the caller on realm
   routes, with API tests: an owner's signed save works, another key's is
   403, a replayed event is refused.
3. `apps/openagents-cli` with `login --key-stdin`, `whoami`, and the read
   verbs (`realm list|show|doc|shot`).
4. The edit verbs and `realm apply`; a `cli-journey` check like
   `forge-journey` that runs make, move, save, a stale save (exit 5), undo,
   publish and fork against staging with a test account.
5. Device login (routes and page; the key stays in the 0600 file).
6. `openagents mcp` and `openagents skill`, then a run where Claude Code edits a
   realm through MCP from a fresh login.
7. Release builds and the install script.

## Decisions for the owner

1. **Name.** `openagents` only, or also a short `oa` alias?
2. **Agent keys.** In v1 a linked key acts as the whole account. Should a key
   be limited to some realms or to read-only from the start (a scope on its
   `auth.identities` row), or later?
3. **Phone approval.** Should the iOS app also approve a CLI login, as it
   approves the web QR today?
4. **Linux keys.** Raw key file at mode 0600 by default (like SSH keys), or
   always NIP-49 encrypted with a passphrase?
