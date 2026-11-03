# openagents CLI

The OpenAgents command line, in Coax. The plan for v1.0.0 is
[docs/cli/2026-11-01-openagents-cli-v1.md](../../docs/cli/2026-11-01-openagents-cli-v1.md).
Its domains include `nostr`, your NIP-06 identity and a relay client;
`pylon`, which starts and stops this computer's Pylon; `project`, the
project queue at openagents.com/projects/SLUG; `transcribe`, speech to
text on this computer; and `labs`, repeatable shots, clips, decks, cuts and
playtests.

## Build

```sh
coax/build.sh                    # the compiler, once
apps/openagents-cli/build.sh     # about 10 seconds; writes apps/openagents-cli/build/openagents
ln -sf "$PWD/apps/openagents-cli/build/openagents" ~/.local/bin/openagents
```

macOS and Linux. The build needs the Coax compiler and the system C
compiler; on Linux, TLS comes from the system's OpenSSL 3 (on Ubuntu and
Debian, `apt install libssl-dev`), or from OpenSSL through Nix where there is
none and on NixOS ([coax/platform/tls-env.sh](../../coax/platform/tls-env.sh);
`coax/web/tool.sh coax/tests/net/tls-env.coax` checks which it picks). A binary
linked with Nix's OpenSSL carries Nix's glibc and does not start on an ordinary
distribution.

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

- A profile with no origin of its own (it never logged in) keeps the first
  `--origin` that works for `whoami`, `account`, `access`, `realm` or `admin`,
  the way `login` does, and says so on stderr; a later command without
  `--origin` goes to that server, never silently to production. A saved
  origin is never replaced this way, and `OPENAGENTS_NSEC` runs store
  nothing. A server's error names the server:
  `openagents: https://openagents.com: invite only` (`--json` keeps `error`
  as the server's words and adds `origin`).

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
openagents realm list                              # your realms: SLUG VISIBILITY REV PUBLISHED NAME columns (--json: the API's objects)
openagents realm show SLUG                         # its spec lines and its page
openagents realm new "a koi garden at dusk"        # plan it from words and make it, private
openagents realm new "WORDS" --style flat --time night --camera top --ground snow   # fix the setup; the rest is the model's pick
openagents realm new --from PLAN                   # make a plan you have
openagents realm visibility SLUG private|unlisted|public
openagents realm publish SLUG [--rev N]            # visitors see this revision
openagents realm fork SLUG [--name NAME]           # a copy you own, private
openagents realm doc SLUG                          # the versioned document and SHA-256
openagents realm asks SLUG | proposals SLUG | panes SLUG
openagents realm switch SLUG ROW on|off [--rev N]
openagents realm revise SLUG "WORDS" [--rev N]
openagents realm ask SLUG "WORDS" [--rev N]         # or --doc FILE, at most 360 KiB
openagents realm ask-delete SLUG ID [--rev N]
openagents realm ask-done SLUG ID --commit HASH [--rev N]
openagents realm row-delete SLUG ROW [--rev N]
openagents realm move SLUG ENTITY X Z [--rev N]     # trees:3; metres within +/-40, up to two decimals
openagents realm place SLUG THING X Z [--yaw DEG] [--rev N]
openagents realm settings SLUG --tools build,talk [--rev N]
openagents realm lore SLUG WHO "TEXT" [--rev N]     # empty forgets it; at most 400 bytes
openagents realm quest SLUG GIVER --ask A --step S1 --step S2 --reward R [--rev N]
openagents realm accept SLUG ID | decline SLUG ID [--rev N]
openagents realm undo SLUG [--rev N]
openagents realm apply SLUG FILE [--rev N]          # at most 64 KiB / 256 JSON-line operations
```

`realm apply`'s FILE is one JSON object a line, `{"op":OP, ...}` with the
op's fields (`?` optional): `move entity x z`, `switch row on`, `place thing x
z yaw?`, `revise prompt`, `ask words` or `ask doc`, `ask-delete id`, `ask-done
id commit`, `row-delete row`, `settings tools`, `lore who text`, `quest giver
ask steps reward?` (`steps` an array of at most three strings), `accept id`,
`decline id`, `undo`, `publish listed?`, `visibility visibility`. Numbers are
JSON numbers, `on` and `listed` are `true` or `false`, and a revision goes in
`--rev`, never in a line. `openagents help` prints the same list. A bad line
is named with its field: `line 2: field "on" must be true or false`.

Setup: `--style lowpoly|painterly|flat`, `--time day|dawn|dusk|night`,
`--camera third|first|top|side`, `--ground
meadow|forest|desert|snow|stone|beach` (the server's `verse.builder_setup`).
They go with words, not `--from`. There is no route to change a made realm's
setup, its palette or an anime style yet (see the spec's "Not built yet").

The catalogue in `commands.coax` supplies realm help and checks bounded
arguments before requests. Each edit reads the current revision unless
`--rev N` names one. A stale revision returns exit 5 and the revision now.
Move saves preserve every existing move; adjacent batch moves share one
save and undo step. Other batch operations run separately. The first refusal
stops the batch and reports `applied`, the failing line and the known revision.
A batch is not atomic.

New edit routes require migration 0102 and the `edit-v1` server family;
older servers refuse them before effects, including in rolling deployments.
Side-table edits (lore, quests and asks) consume revisions for ordering;
their undo remains the existing realm document's scope. The broader
invertible edit log is #12629.

The end-to-end check drives the built CLI with a throwaway in-memory identity:
`coax/web/tool.sh cli-journey --cli apps/openagents-cli/build/openagents --fixture`,
then the same command with `--origin https://staging.openagents.com`.
It refuses production origins.

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

## Transcribe

```shell
openagents transcribe clip.wav                    # default: local Parakeet, automatic language
openagents transcribe clip.wav --json             # versioned file metadata, text and global token timing
openagents transcribe talk.wav --srt > talk.srt    # or --vtt
openagents transcribe models                      # pinned Parakeet directory and file presence
openagents transcribe memo.m4a --model-dir /path/to/pinned-parakeet --device gpu
openagents transcribe talk.wav --backend psionic --model small --language auto --srt
openagents transcribe fetch medium --backend psionic   # explicit legacy Whisper download
```

- The default is Psionic's pinned original Parakeet TDT v3 model. Its Metal
  encoder and Coax TDT decoder run locally; no Whisper tool or model is used
  and an error never chooses another backend. Install the exact model data
  using [the pinned setup commands](../../psionic/docs/parakeet.md).
- The default directory is
  `~/.cache/openagents/models/parakeet/parakeet-tdt-0.6b-v3-541d1f99`;
  `OPENAGENTS_PARAKEET_MODEL` or `--model-dir DIR` selects another copy of
  the same authenticated files. Transcription never downloads a model.
- Text, JSON, SRT and WebVTT use complete nonoverlapping model windows of
  320..327,520 mono 16 kHz samples. A short last remainder borrows from the
  preceding window; no sample is dropped, repeated or invented as padding.
  The file limit is 4,096 windows; the command also refuses a result exceeding
  14 minutes of child execution or 4 MiB of captured transcript output.
- JSON is versioned as `openagents.parakeet.file.v1`. Each window records its
  exact sample range, tokens, local TDT frames and global token sample times.
  Single-window JSON retains the earlier top-level token/frame metadata.
  One nonempty model window becomes one subtitle cue; cue times bound TDT
  emission, not acoustic word alignment. Independent windows reset context.
- Forced language, `--model`, `--threads`, `--words` and `--prompt` remain
  explicit refusals. Use `--language auto` or omit it.
- A malformed/truncated WAV refuses. Cancellation or model failure before
  final output leaves stdout empty; output-device failure or cancellation
  during final publication can leave partial bytes and returns nonzero.
- WAV decoding/resampling is Coax. Other audio/video containers use the
  explicit local ffmpeg conversion boundary; it is not a speech provider.
- `--backend psionic` retains the existing Whisper CLI contract and model
  cache as explicit compatibility. `--backend whispercpp` explicitly invokes
  the independent Whisper oracle. Their historical measurements remain in
  [the transcription record](../../psionic/docs/transcribe.md).
- Status/errors go to stderr; only successful transcript output goes to stdout.

## Dictate

```shell
openagents dictate                       # listen now; Enter stops, Esc or Ctrl-C cancels; prints the text
openagents dictate --seconds 10 --json   # stop by itself after 10 s; one JSON result
openagents dictate --wait-for-enter      # load and open first, then Enter starts and Enter stops
openagents dictate --events              # JSON lines: status events, then one result
openagents dictate --device cpu --capture alsa --input hw:1   # Linux: name the recognizer device, recorder and input
```

- Your microphone to text on this computer, with the same pinned Parakeet
  model as `transcribe` (`--model-dir DIR` or `OPENAGENTS_PARAKEET_MODEL`;
  nothing is downloaded). The model loads first; the microphone opens only
  when recognition is ready and closes as soon as the clip is captured.
- Nothing is pasted, typed, sent or saved: the text goes to stdout, and the
  audio stays in memory and is cleared after recognition. The shared
  controller from Coder for Mac decides the timeline (450 ms pre-roll with
  `--wait-for-enter`, 300 ms minimum, 200 ms tail, 60 s cap), silence,
  tickets and the exact model segments (at most 3 calls of up to 327,520
  samples); the resident engine's utterance buffers are cleared after each call.
- Controls: Enter, Escape, Ctrl-C (or SIGINT/SIGTERM from a parent). On a
  terminal the keys are read without echo; other keys and end of input do
  nothing. `--seconds N` (1 to 60) stops by itself.
- Recognition device: `--device gpu|cpu`; the default is the GPU (Metal) on
  macOS and the CPU on Linux. A named device never falls back to the other.
  Parakeet on a Linux CPU takes about 8 to 15 times the audio's length.
- Capture: CoreAudio on macOS (`--input UID` names one input; a missing one
  is an error, never the default instead). On Linux the command runs a
  recorder that writes mono 32-bit float to a pipe: `pw-record` (PipeWire,
  the default), `parec` (`--capture pulse`) or `arecord` (`--capture alsa`);
  `--input` is its target, and PipeWire is told not to fall back.
- `--json` prints one object, `"schema":"openagents.dictation.v1"`, with
  `"type"` `final` (`text`, `samples`, `audio_ms`, `segments`, `limited`),
  `no_speech` (`reason`: `silence` or `too_short`), `cancelled` or `error`
  (`error`: `permission`, `model_missing`, `model`, `device`, `program`,
  `capture`, `format`, `recognition` or `too_long`, and a `message`), plus
  `exit`, `recognizer`, `model`, `revision`, `device` and `capture`.
  `--events` prints `"type":"status"` lines first (`loading_model`,
  `opening_input`, `ready`, `listening`, `finishing`, `recognizing`): they
  carry a state, never words. There are no partial transcripts yet; the one
  `final` is the whole clip's text.
- Exit codes: 0 text, 9 no speech, 130 cancelled, 4 microphone permission
  denied, 6 model or input not found, 8 recorder program missing, 1 any other
  failure, 2 usage. In text mode only the final text goes to stdout.
- `openagents dictate` runs `build/openagents-dictate` beside it (built by
  `build.sh`): the dictation host links Psionic's engine, whose generated C
  exports names the CLI's signing code also exports.
- Checks use recorded audio through a test capture backend (`--capture
  fixture` with `OPENAGENTS_DICTATE_FIXTURE=FILE.wav`), never your microphone:
  `coax/web/tool.sh apps/openagents-cli/tests/dictate-check.coax [MODEL_DIR]`.
  [Record](../../docs/verification/dictate-cli-2026-11-02/README.md).

## Labs

`labs` makes files from pages, cut lists and games, the same way every time
([the spec](../../docs/cli/2026-11-01-openagents-labs.md)). Chrome is always
headless and muted. Each command checks its input first, writes a label
beside its output (`OUT.json`: how it was made, and the digest of
everything it read), and does nothing when the label says the output is
already there. `--fresh` makes it again.

```sh
openagents labs shot /realms/orbit-league home.png --phone        # a PNG of a page; realm shot SLUG is the short name
openagents labs clip /realms/hold-the-stair stair.mp4 --stage highlight --seconds 6
openagents labs deck docs/decks/demo.deck demo.pdf                # --check prints slides, scenes and images
openagents labs cut episode.cut episode.mp4 --quality review      # .cut lines or cut.v1 JSON; --check, --captions
openagents labs frames episode.mp4 frames --strip 8               # or --at 1.5,4 or --every 2
openagents labs compare a.mp4 b.mp4 side.mp4                      # what differs in the labels, and side by side
openagents labs board site.board shots                            # every page at each size and theme, with index.html
openagents labs playtest icecream --set walk=3 --against base.json
```

- The cut format is in [docs/cli/cut-format.md](../../docs/cli/cut-format.md).
  Titles and captions are drawn in Filament. `--captions` runs
  `openagents transcribe --json` on the voice. `--quality master` adds a
  ProRes `.mov` beside the MP4.
- `playtest icecream` plays Ice Cream Man's bot on 24 seeded houses (as
  Game Lab did), prints each run and a summary, and `--json` writes a result
  that `--against` and `labs compare` read. `--tunables` lists what `--set`
  can change.
- A `.board` file is `board NAME`, `base URL`, `sizes desktop phone`,
  `themes light dark`, and `page PATH NAME [wait=MS]` lines.
- Needs Chrome and ffmpeg; exit 8 names the install command when one is
  missing. `--as-me` (a page signed in as the profile) is not built yet.

## Output and exit codes

Text is `key value` lines. `--json` prints one object with `"ok"`. Exit codes:
0 done, 1 failed, 2 usage, 3 no identity or key, 4 forbidden, 5 conflict
(changed since), 6 not found, 7 slow down, 8 a needed program (Chrome,
ffmpeg, whisper-cli, pw-record) is missing. `dictate` adds 9 no speech and
130 cancelled, and prints its own JSON result (see Dictate).

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
| `realm-play.coax` | a seat in a running realm: `realm play`, `look`, `go`, `say`, `use`, `bye`, `leave`, and the detached holder (`seat-hold`) that keeps the page open in headless Chrome (`coax/lib/media/realm_seat.coax`) |
| `admin.coax` | the `admin` domain: invites, requests, creators, flags, realms, admin keys |
| `realm_rules.coax` | `#authority`: slugs, visibilities, flags, counts, keys and account IDs, origins, spec lines, access rows, a plan's board |
| `project-rules.coax` | `#authority`: hashes, slugs, standing, claims, plans, ranks, the order, relay paging |
| `rules.coax` | `#authority`: profile names, the stored key's format, words as typed, hex, exit codes, the Pylon log's facts |
| `transcribe.coax` | the `transcribe` domain: fetch a model, decode with ffmpeg, run the backend, print the transcript |
| `transcribe-rules.coax` | model names and files, formats, whisper.cpp's segment lines ([segments.coax](../../psionic/lib/whisper/segments.coax) writes text, SRT, WebVTT and JSON) |
| `dictate-launch.coax` | the `dictate` domain: its usage, and running `build/openagents-dictate` |
| `dictate-host.coax` | the dictation host's `tool_main` |
| `dictate.coax` | one dictation: model, input, listening through the shared controller ([dictation.coax](../shared/dictation.coax)), segments, the text or JSON result; the recorded-audio test backend |
| `dictate-rules.coax` | `#authority`: capture backends, the device, `--seconds`, input names, recorder arguments, control keys, a stalled input, results and exit codes |
| `dictate-native.c` | effects: the terminal and signals, the recorder pipe, the shared controller's C face, the resident Parakeet engine on a worker thread |
| `dictate-mac.m` | effects on macOS: microphone permission, CoreAudio inputs and an AudioQueue |
| `labs.coax` | the `labs` domain: shot, clip, deck, frames, compare, cut, board and playtest, with Chrome ([media.coax](../../coax/lib/media/media.coax)) and ffmpeg |
| `labs-rules.coax` | the `.cut` and `.board` formats and their checks, cut.v1 JSON, sizes and qualities, times, the playtest summary, Ice Cream Man's headless run |
| `cli-native.c` | effects: Keychain, 0600 files, stdin without echo, the clock, the host name, process liveness, the executable's path |
| `build.sh` | compiles `main.coax` to C and links it with the host, sockets and TLS; links the dictation host with the controller and Psionic Parakeet |

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
coax/bin/coax test apps/openagents-cli/transcribe-rules.coax --library
coax/bin/coax test apps/openagents-cli/labs-rules.coax --library
coax/bin/coax test apps/openagents-cli/dictate-rules.coax --library
coax/bin/coax mutate apps/openagents-cli/dictate-rules.coax --library    # 256 mutants, 0 survivors (2026-11-02)
coax/web/tool.sh apps/openagents-cli/tests/dictate-check.coax [MODEL_DIR]   # the real CLI on recorded audio
coax/bin/coax mutate apps/openagents-cli/account_rules.coax --library   # 0 survivors (2026-11-01)
coax/web/api/cli-auth-test.sh https://staging.openagents.com     # the server side of login, as a Coax program
```
