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
```

`inbox` reads kind-1059 gift wraps addressed to the triage key from
`wss://relay.openagents.com`, authenticating with NIP-42 because the relay
serves a wrap only to the reader its `p` tag names. It opens each with the
triage key, and keeps a report only when the log has none with the same
gift wrap ID, message ID, or content digest. It never guesses that two
different reports describe the same problem: that is the triager's call,
recorded with `decide --decision duplicate`.

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

Before appending, the command checks the entry against the log: a decision
or filing names a received report that is still waiting; each issue has one
accepted reporter (a second report on it is a duplicate); the triager isn't
the tester and the moderator isn't the tester; a session script is recorded
once per tester; and a verification names an issue filed from a report.

`log --acceptances` joins the log into the accepted contributions an award
needs: the tester's key, the contribution, the issue or script, the
severity, the build, the acceptance time, and for a verified fix the fixing
build. Once the NIP-XP `playtest` rule and the playtest referee key exist
([#9885](https://github.com/OpenAgentsInc/openagents/issues/9885)), awards
are signed from these rows.
