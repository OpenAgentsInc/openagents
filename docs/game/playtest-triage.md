# Playtest triage inbox

The triage inbox turns private playtest reports into drafted GitHub issues
and keeps the triage log, the record of every acceptance that later backs a
playtest award. It is the `openagents playtest` command group
(`crates/openagents-cli/src/playtest.rs`) over the `playtest` crate's
`report` and `triage` modules. The program it serves is
[Playtesting program](playtesting.md), section
[Triage](playtesting.md#triage-from-report-to-the-next-build); the issue is
[#9884](https://github.com/OpenAgentsInc/openagents/issues/9884).

## Status

Implemented. Reports reach the inbox only once the triage key exists and a
build carries its public key as `playtest::TRIAGE_KEY`; until then the app
keeps reports on the phone. The owner steps for creating the key are in the
workspace's `NEEDS_OWNER.md`.

## Files

Everything lives in `~/.openagents/playtest/` (`OPENAGENTS_PLAYTEST_HOME`
overrides it), created `0700`, with each file `0600`:

| File | What it holds |
| --- | --- |
| `log.jsonl` | The triage log: one JSON object per line, appended, never rewritten. |
| `drafts/CODE.md` | The issue draft: `# Title`, then the body. Edit it before filing. |
| `drafts/CODE.json` | The draft's labels, the tester's key, and the report digest. |
| `drafts/CODE.report.json` | The whole private report, without the screenshot. |
| `drafts/CODE.jpg` | The screenshot, when the tester attached one. Never published. |

The triage key is a separate file (for example `~/.openagents/playtest/triage.key`)
holding an `nsec`. The command refuses a key file that anyone but its owner
can read, and never prints the secret.

## The loop

```sh
# Once, by the owner: create the key and note its npub.
openagents playtest keygen --out ~/.openagents/playtest/triage.key

# Daily: read new reports and draft an issue for each.
openagents playtest inbox --triage-key ~/.openagents/playtest/triage.key

# Look at what is waiting, edit drafts/CODE.md, then decide each one.
openagents playtest log --pending
openagents playtest file PT-1A2B3C4D --contribution bug --severity p2           # dry run
openagents playtest file PT-1A2B3C4D --contribution bug --severity p2 --approve # gh issue create
openagents playtest file PT-1A2B3C4D --contribution feedback --issue 9950      # an issue a person filed or kept
openagents playtest decide PT-2B3C4D5E --decision duplicate --issue 9950 --reason "Same as #9950."

# When the fixing build ships and the reporter checks it.
openagents playtest verify --issue 9950 --fix-build "1.0.0 (16)" --verified yes

# A completed session script or diary week with an accepted report.
openagents playtest session --tester NPUB --script session-2 --format moderated \
  --build "1.0.0 (15)" --moderator NPUB

# Which accepted contributions back which awards.
openagents playtest log --acceptances --json

# TestFlight feedback from App Store Connect (screenshots and crashes).
openagents playtest testflight --asc-env ~/work/.secrets/appstoreconnect.env --since 2026-09-29
```

`inbox` reads kind-1059 gift wraps addressed to the triage key from
`wss://relay.openagents.com`, authenticating with NIP-42 because the relay
serves a wrap only to the reader its `p` tag names. It opens each with the
triage key, and keeps a report only when the log has none with the same
gift wrap ID, message ID, or content digest. It never guesses that two
different reports describe the same problem: that is the triager's call,
recorded with `decide --decision duplicate`.

## TestFlight feedback

`testflight` reads what TestFlight testers send with **Send Beta Feedback**
or a shared screenshot, and crashes they chose to send, from App Store
Connect ([#9905](https://github.com/OpenAgentsInc/openagents/issues/9905)).
It signs a 20-minute App Store Connect API token (ES256) with the team's API
key: `--asc-env FILE` names a file with `ASC_API_KEY_ID`, `ASC_API_ISSUER_ID`,
and `ASC_API_PRIVATE_KEY_PATH` (the `.p8`), or the same variables come from
the environment. The key is never printed. `--app` defaults to the
OpenAgents app, `6748620735`; `--since` takes an ISO date and skips older
submissions.

Each submission the log doesn't hold yet gets a `TF-1A2B3C4D` code, a draft,
and a `testflight` log entry (code, submission ID, `screenshot` or `crash`,
build, and send time). Its screenshots (`drafts/CODE.jpg`,
`drafts/CODE-2.jpg`, …), crash log (`drafts/CODE.crash.txt`), and the
tester's comment (`drafts/CODE.testflight.json`) stay in the private drafts
folder. Screenshot links expire after about a week, and old crash logs are
no longer served, so the command counts what it couldn't download.

The tester's Apple email and name are never read into a file or the log. A
TestFlight draft never quotes the comment, because the tester wrote it to
OpenAgents through Apple, not for a public issue: `file --approve` refuses
it until a person writes the title and text in their own words. A TestFlight
tester has no Nostr key, so a filed TestFlight entry is recorded but isn't
among `log --acceptances` and backs no playtest award.

## Drafts

A draft is written in the repository's issue style: the surface and a
title, the report code, the build (`1.0.0 (15)`), the platform, OS, and
device, the tab and screen, the kind, and a severity placeholder. It
carries the labels `playtest`, `build:1.0.0-15`, and `area:coder`,
`area:grid`, `area:wallet`, or `area:account`; `file --approve` creates a
missing build or area label.

- The tester's words appear, as quotes, only when they allowed quoting.
  Otherwise the draft holds placeholders, and `file --approve` refuses it
  until a person writes the title and text in their own words.
- No screenshot or session log is embedded. The draft says they are
  attached to the private report. The app never attaches a screenshot from
  the Wallet or a key screen, and the report format refuses one.
- The tester's key isn't in the issue; it is in the triage log.

## The triage log

Each line is one event:

| `event` | Fields | Meaning |
| --- | --- | --- |
| `received` | code, rumor and wrap IDs, digest, tester, build, platform, tab, route, kind | A report arrived and was drafted. Earns nothing. |
| `filed` | code, issue, contribution (`feedback`, `bug`, `design`), severity, triager | Accepted as a new `playtest` issue. `at` is the acceptance time. |
| `decided` | code, decision (`duplicate`, `not-reproducible`, `design`, `idea`, `declined`), reason, issue | Not filed as a new issue. Earns nothing. |
| `verified` | issue, fix build, `yes`, `no`, or `unverified` | The reporter checked the fix on the fixing build. |
| `session` | tester, script, format (`unmoderated`, `moderated`, `group`, `diary`), build, code, moderator | A completed script or diary week with an accepted report. |
| `testflight` | code, submission ID, source (`screenshot` or `crash`), build, created | TestFlight feedback arrived from App Store Connect and was drafted. Earns nothing; filing it backs no award. |

Before appending, the command checks the entry against the log: a decision
or filing names a received report that is still waiting; each issue has one
accepted reporter (a second report on it is a duplicate); the triager isn't
the tester and the moderator isn't the tester; a session script is recorded
once per tester; and a verification names an issue filed from a report.

`log --acceptances` joins the log into the accepted contributions an award
needs: the tester's key, the contribution, the issue or script, the
severity, the build, the acceptance time, and for a verified fix the fixing
build.

## Awards

`award` signs the NIP-XP `playtest` award for one accepted contribution
([#9906](https://github.com/OpenAgentsInc/openagents/issues/9906)):

```sh
openagents playtest award --issue 9950 --quest playtest-s1.bug@1            # dry run
openagents playtest award --issue 9950 --quest playtest-s1.bug@1 --publish
openagents playtest award --issue 9950 --verified --quest playtest-s1.verified-fix@1 --publish
openagents playtest award --script session-2 --tester NPUB --quest playtest-s1.session-2@1 \
  --session RECORD-ID --publish
```

It names the acceptance by report code, `--issue` (with `--verified` for a
verified fix), or `--script` and `--tester`, then reads from the relay
(`wss://relay.openagents.com` unless `--relay`) the referee's quest at
`--quest`, the tester's public playtest report (kind `3197`, which the app
publishes with every sent report, found by the tester's key and the accepted
report's digest), the session record named by `--session`, and the referee's
awards and revocations. It builds the award from the triage log's row, with
the issue as `OWNER/REPO#N` (`--repo`), the severity, the triager
(`--triager` if the log has none), and `--commit` for a design change;
checks it against the NIP-XP `playtest` rule; and refuses a key that
already has a live award or a quest version at its `max_awards`. Without
`--publish` it prints the signed award and publishes nothing.

The command is off until the playtest referee key exists: while
`verse::xp::PLAYTEST_REFEREE` is unset it refuses before reading anything,
and afterwards it signs only with that key (`--referee-key`, by default
`~/.openagents/nostr/playtest-referee-key`). A TestFlight entry has no
tester key, so it can't be awarded.
