# Program status (OSC 7501): what OpenAgents does about it

Status: proposal, October 6, 2026. Nothing here is implemented. This page
decides how OpenAgents emits, parses, carries, and shows the Program Status
Protocol, and plans the work. It tracks spec revision 0.2.

## Summary

- **Adopt it on both sides.** Our programs report their state with OSC 7501,
  and our terminals, host, and clients read it. It is the one in-band status
  channel that works through a PTY, SSH, containers, and our own NIP-TERM
  host, so one implementation serves every surface.
- **Typed events stay authoritative where we have them.** Coder V1's JSON
  events, Claude Code's `stream-json`, Codex's `--json`, and ACP updates
  remain the source of truth for the runs we orchestrate. OSC 7501 is what a
  *pane* says about itself; it fills the gap where we have only a terminal.
- **A status record never authorizes anything.** A `blocked` record is
  advisory text from an untrusted program. CONFIRM on an inbox entry focuses
  the pane; it never types an answer.
- **No notifications.** Status shows in the always-visible status area and
  on an anchored inbox page, as the [design principles](design-principles.md)
  require.
- **One new NIP-TERM feature,** `openagents.terminal-status.v1`, carries the
  host's record table to every attached device under the existing rights,
  shares, and pause rules.
- **About 40 agent-hours** in six phases, starting with a dependency-free
  parser crate and conformance fixtures built from the spec's examples and
  limits.

## Sources

- Mitchell Hashimoto, [A terminal protocol for program status](https://mitchellh.com/writing/program-status-osc7501),
  October 6, 2026.
- Superlogical, [Program status protocol](https://www.superlogical.com/rex/docs/build/program-status),
  revision 0.2 (2026-10-06; revision 0.1 was 2026-09-28).

Both were read on 2026-10-06. Mitchell Hashimoto wrote Ghostty and founded
Superlogical, the server-side multiplexer the
[smart terminal specification](smart-terminal.md#superlogical-the-model-this-follows)
follows and the [optional research](2026-10-06-optional-research.md#superlogical-and-libghostty-interop-10696)
reassessed. This is the first protocol Superlogical has published, and it
is ten days old. Treat it as a draft: pin the revision we implement, and
recheck the spec page before each phase.

## The protocol in brief

A program writes `OSC 7501 ; pairs ST` to its terminal. `ST` is `ESC \` or
`BEL`. Pairs are `key=value` joined by `:`; keys match `[a-z]+` and values
match `[A-Za-z0-9_.,+/=-]*`, so a value never needs escaping.

| Key | Values | Rules |
| --- | --- | --- |
| `state` | `idle`, `working`, `done`, `blocked`, `error`, `clear` | Required. An unknown state rejects the report. |
| `id` | Path of segments `[A-Za-z0-9_.+-]{1,32}` joined by `/` | At most 8 levels and 128 bytes. Absent means the root record. |
| `kind` | `permission`, `question`, `auth` | Only with `blocked`; ignored otherwise. |
| `progress` | Integer 0 to 100 | With `working` or `blocked`. Non-numeric is treated as absent. |
| `app` | `[A-Za-z0-9_.+-]{1,32}` | A stable program name. A record without one inherits its nearest ancestor's. |
| `title` | Base64 UTF-8 | At most 256 bytes encoded, 192 decoded. |
| `msg` | Base64 UTF-8, one line | At most 2,732 bytes encoded, 2,048 decoded. |

Rules a terminal must follow:

- **Replacement.** Each report replaces its record whole; a key the report
  omits is removed.
- **Lifetime.** When the attached process exits or a new prompt begins
  (OSC 133 `A`), the terminal drops `working` and `blocked` records and may
  drop `idle` ones. `done` and `error` survive. A full reset (`RIS`) removes
  every record; a soft reset (`DECSTR`) and switching screens do not.
- **Clearing.** `state=clear` removes the addressed record and its
  descendants; without an `id`, it removes every record.
- **Errors.** A malformed pair is skipped and an unknown key ignored; the
  last duplicate wins. Invalid base64, a control character in decoded text,
  or any exceeded limit discards the whole report.
- **Limits.** At most 4,096 bytes from OSC to ST, and keys of at most 16
  bytes. A terminal keeps up to 256 records and must support at least 64;
  above its capacity it evicts the least recently updated record.
- **Detection.** A program sends `OSC 7501 ; ? ST`, and a supporting
  terminal answers with the same bytes. A program can follow the query with
  `CSI c` (primary device attributes), which every terminal answers, so
  whichever reply arrives first decides. The terminfo capability `Pst`
  (`\E]7501;%p1%s\E\\`) advertises support, but a program must still query
  when it is absent, because entries go stale over SSH and in multiplexers.
- **Security.** Everything in a report is untrusted. Refuse control
  characters, never interpret `msg` or `title` as markup, disarm bidirectional
  overrides and invisible formatting outside the grid, never echo a report's
  content back to the program, show which terminal each record came from,
  and rate-limit any external effect.

The argument for it, in the post's words and ours: more than 250 agent
orchestrators each guess whether Claude Code is working, blocked, or done.
They match screen text and window titles, which breaks with every release
of the agent, or they ask each program to integrate with their own socket
API, which is O(N) work per tool and fails over SSH and in containers. An
in-band sequence reaches whatever terminal the program's bytes reach.
libghostty and Rex implement it, and Terraform, Claude Code, Codex, and
Homebrew have proofs of concept; none of those proofs of concept ships in a
release yet, so our consumers must handle its absence.

## Where we stand

| Area | Today | Gap |
| --- | --- | --- |
| Emulator, `crates/coder-vt` | `osc_dispatch` in `src/lib.rs` handles OSC 0 and 2 (title), 8 (links), and 52 (clipboard writes). `src/shell.rs` parses OSC 133, OSC 7, and the hook's private OSC 777 marks into bounded `Event`s (primary screen only). An OSC string past 1 MiB (`MAX_CLIPBOARD`) is abandoned. `src/snapshot.rs` writes the NIP-TERM snapshot stream (`openagents.terminal-snapshot.v1`), which carries no shell marks, bells, or side effects. | Every other OSC, OSC 7501 included, is dropped silently. |
| Host authority | `coder_vt::Authority` (`src/authority.rs`) parses each host terminal's output once, answers queries, reports effects, and keeps the block journal (`src/journal.rs`, #10656: at most 256 blocks, never the output). | No status table. |
| NIP-TERM, `crates/coder-pty` | [Extensions](../../nips/openagents/NIP-TERM.md#extensions) for snapshots, blocks, sessions, proposals, effects, the typist, and shares; wire types in `src/ext.rs`, fixtures in `fixtures/nip-term-ext.json`. | No status record type. |
| Terminal app, `crates/terminal-core` | The paper sheet (`src/paper.rs`): a three-row status area (`DIR`, `GIT`, `EXIT`; `REQUEST`, `QUEUE`, `PENDING`, `DOOR`, `LOAD`, `TIME`; `CONTEXT`), the transcript, one input line with the smart caret, and the key strip. Blocks from OSC 133 (`src/blocks.rs`). F1 to F17 are taken. | A pane's program state is invisible unless that pane is focused. |
| Other clients | Phone (`crates/coder-computers/src/terminal`, drawn by `crates/terminal-gfx/src/phone.rs`), browser (`crates/coder-browser/src/workbench.rs`, `crates/everglade-web/src/terminal.rs`), Verse (`crates/verse/src/terminal.rs`), the TTY multiplexer (`crates/terminal-mux`), and paired-host mounts (`crates/terminal-remote`). | Same as above. |
| Workshop agent and seats | Alice (`crates/verse/src/workshop.rs`, [workshop agent](../verse/workshop-agent.md)) and the [Agent Studio](../verse/agent-studio.md) seats run Coder V1 turns. The host reads Coder's NDJSON events (`crates/coder/src/task/coder_v1.rs`, `agent_coder.rs`) into `coder_access::studio::Activity`, which nameplates and the boards (`crates/verse-zone-everglade/src/zones/everglade/boards.rs`) draw. | Correct for Coder V1; a seat or pane running any other program has no status. |
| Orchestration | `coder-delegate` drives Claude Code with `stream-json`, Codex with `exec --json`, and OpenCode with `--format json` (`src/adapter.rs`); `acp-client` reads ACP `session/update` and permission requests; the Microcoder loop (`crates/microcoder-loop`) owns its own events. | None of these needs a heuristic, because none runs in a PTY. An agent a person starts by hand in a pane (`claude`, `codex`) reports nothing we read. |
| Emitters | Nothing in the repository writes OSC 7501. Coder V1, the older `coder`, `openagents terminal`, `microcoder`, `openagents lease build`, `scripts/grid-soak.sh`, `scripts/test-soak.sh`, and `retail-qualify` print state only as text. | Terminals around them (Ghostty, Rex, ours) cannot tell they are waiting. |

## Decision 1: emit from our programs

### Where the code lives

Add `crates/program-status`: no dependencies beyond `std` and a base64
routine, so `coder-new`, `coder`, `microcoder`, `openagents-cli`, and
`coder-vt` can all link it. It holds:

- `Report` and `Record` types, the encoder, and the parser (one for both
  sides, so what we write is exactly what we accept).
- `Table`, the record table with the spec's limits and lifetime rules.
- `detect`, the query-and-DA1 probe with a timeout.
- `SPEC_REVISION = "0.2"`.

### When a program emits

- Only when the stream it writes the sequence to is a terminal. A run with
  `--json`, `-p`, piped output, or a child process of the host never emits;
  JSON consumers keep their typed events.
- Only after detection says yes: `Pst` in terminfo, or a `?` reply before the
  DA1 reply. Full-screen programs probe once at startup, before their event
  loop reads input.
  Line-mode commands probe only when they will run long enough to matter
  (more than about two seconds), with a 200 ms timeout.
- `OPENAGENTS_PROGRAM_STATUS=0` turns it off; `=1` skips detection.
- Through the same writer as the program's frames, never from another
  thread mid-frame.
- On exit, a final `done` or `error` record, so the result survives the
  next prompt. A program that is interrupted writes `state=clear` for its
  `working` records if it can.

### State mapping

| Our state | `state` | `kind` | Example `msg` |
| --- | --- | --- | --- |
| Composer waiting for the first message | `idle` | | |
| Model streaming, tool or command running, Jev judging | `working` | | `Running cargo test -p coder-vt` |
| Coder V1 approval event (CONFIRM or REJECT a command) | `blocked` | `permission` | `Approve: cargo publish -p coder?` |
| A question for the person, answered in the composer | `blocked` | `question` | `Which branch should I use?` |
| A provider needs a login, an expired token, a missing credential | `blocked` | `auth` | `Sign in to Codex` |
| `openagents lease grant screen` asking to confirm | `blocked` | `permission` | |
| Waiting in the lease queue | `working` | | `Waiting for a build slot, 2 ahead` |
| Turn finished, result ready | `done` | | `Tests pass, 3 files changed` |
| Turn failed, every provider at its limit, a check failed | `error` | | the refusal's one-line reason |
| A run the person stopped | `idle` | | `Stopped` |

`progress` is sent only when a number is real: Jev's completion estimate on
a Coder run (`step N, about X% done`), soak elapsed against its planned
duration, qualification steps done against planned. Never invent one.

The workshop's `Activity` maps the same way: `Idle` and `Paused` to `idle`;
`Reading`, `Editing`, `Running`, `Testing`, `Judging`, and `Thinking` to
`working`; `Waiting` to `blocked` with the kind of what it waits on;
`Done` to `done`; `Failed` to `error`. Our `Activity::Blocked` (a missing
grant or no provider with capacity) is *not* the spec's `blocked`, which
means waiting on a person at this terminal: a missing login maps to
`blocked` with `kind=auth`, and no capacity maps to `error`.

### Record IDs

A program that runs one thing uses the root record and sets `app`. A
program that runs several uses hierarchical IDs below it, so a parent stays
`working` while one child is `blocked`:

| Program | `app` | IDs |
| --- | --- | --- |
| Coder V1 (`openagents coder`) | `coder` | Root for the session; `coder/<session>/<delegation>` for each delegation, such as `coder/task-123/microcoder` or `coder/task-123/claude-code` |
| Older `coder` and `openagents terminal` | `coder`, `openagents` | Root for the conversation; `coder/<run>` for each run in the rail |
| `microcoder` run directly | `microcoder` | Root; `microcoder/<task>/<provider>` across a failover |
| `openagents chat work --issues`, the issue flow | `openagents` | `issue/<number>/<stage>` |
| `openagents lease build`, `openagents lease RESOURCE` | `lease` | Root |
| `scripts/grid-soak.sh`, `scripts/test-soak.sh` | `soak` | Root; `soak/<phase>` |
| `retail-qualify` | `qualify` | Root; `qualify/<step>` |
| Alice and studio seats driving a pane | `coder` | As Coder V1, which is what runs in the pane |

Segments are sanitized to the spec's alphabet and cut to 32 bytes. Emitters
use at most five levels, which leaves three for the multiplexers below to
prefix a host and a pane.

## Decision 2: consume it everywhere we draw a terminal

### Emulator: `coder-vt`

- Parse `7501` in `osc_dispatch` with `program_status::Report::parse`, on
  both screens. The OSC string bound for this number drops from 1 MiB to the
  spec's 4,096 bytes; a longer one is abandoned like any other oversized OSC.
- Keep a `Table` per terminal: 256 records, least recently updated evicted,
  except that a `blocked` record is evicted only when nothing else can be.
  This deviates from the spec's plain LRU, and we raise it upstream.
- Apply the lifetime rules from what the emulator already sees: OSC 133 `A`
  from `shell.rs`, `RIS`, and, through `Authority`, the PTY child's exit.
- Answer `OSC 7501 ; ? ST` like the device queries: on the host's
  `Authority` when the attachment uses the effects feature, otherwise in the
  client's `take_replies`. Never echo anything from a report.
- Store decoded text with control characters refused, and expose a `display`
  form that strips bidirectional controls (U+202A to U+202E, U+2066 to
  U+2069, U+200E, U+200F, U+061C) and invisible formatting (U+200B to
  U+200D, U+2060, U+FEFF). The ASCII sheet maps the rest through
  `terminal_core::ascii`.
- Snapshots: under the status feature only, the snapshot stream gains a
  `STATUS` record after `STATE` with the table, so a joining device restores
  status with the screen. The `openagents.terminal-snapshot.v1` stream a
  client gets without the feature is unchanged.

### Terminal app: `terminal-core`

The design follows the principles: anchored, ASCII, no motion, no
notification, state always visible.

- **A state glyph on every pane and tab label,** from the pane's root record
  and the most urgent descendant: `.` idle, `W` working (with `40%` when
  there is progress), `B` blocked (with `P`, `Q`, or `A` for the kind), `D`
  done, `E` error, nothing when the pane has no record. Steady characters;
  no spinner.
- **Counts in the status area,** on the `REQUEST` row: `PANES W2 B1 D3 E0`,
  across every pane on this sheet and, once phase 4 lands, every attached
  host. It is always visible and never hidden.
- **An inbox page** on F18 (F1 help names it, since the strip is full),
  anchored like the rules page: blocked first, then errors, then done,
  oldest first. Each row shows the host, the pane, the record's `app` and
  `id`, its age, and its `msg`, so every record says where it came from.
  UP and DOWN pick; ENTER focuses that pane and is the only action; ESC
  returns. `D` on a done or error row hides it on this device until the
  record changes; hiding sends nothing to the program.
- The status area's last-message field shows nothing from a record; a
  status change rings no bell and raises no OS notification.

### Host and devices: NIP-TERM

The host's `Authority` already parses every byte once, so it owns each
terminal's table, and devices read it rather than parse it themselves. The
NIP-TERM feature in [Decision 4](#decision-4-the-nip-term-extension) carries
it to the phone, the browser, Verse, the TTY multiplexer, and other
computers. Across hosts, each client's paired-host list already holds a
connection per host (`coder-link`), so the workbench and the phone build
one agentic inbox from every host's list read, labeled by host.

### Verse: nameplates and studio boards

- Alice and Coder V1 seats keep their `Activity` from Coder's typed events;
  those are richer than five states and come from our own program.
- A seat or pane that runs anything else (a third-party agent's TUI, a
  person's shell) takes its nameplate state and the desk monitor's line from
  its terminal's status table, mapped back to the nearest `Activity`.
  Without a record, it shows `Idle` with no claim, rather than a guess.
- Status text appears only in the owner's own client, which reads it from
  the host. It never goes into NIP-MV, chat, presence, or any world event.

### Orchestrators

- Typed protocol events first: Coder V1 NDJSON, `stream-json`, `--json`,
  ACP. Nothing changes for the runs `coder-delegate`, the Microcoder loop,
  and `acp-client` drive.
- OSC 7501 second, for any agent that runs in a terminal pane: a seat that
  drives Claude Code's or Codex's TUI, or an agent a person started by hand.
  When the pane's table has a record whose `app` names the agent, it is the
  agent's state.
- Today's detection last: the process exit and the block journal's command
  state. We add no screen-scraping heuristics.

## Decision 3: multiplexer behavior

### Forward or aggregate

- **Inside our host,** each terminal has its own table. Nothing merges
  tables; the list read and the inbox aggregate them for display only.
- **Out to an outer terminal.** When `terminal-mux` or `openagents computer
  shell` runs inside Ghostty, Rex, or another terminal that answers the
  query, it re-emits one record per pane: the pane's most urgent state under
  the ID `<host>/<pane>`, with `title` naming the host and pane, and the
  pane's root `app`. It forwards only the summary, never a child record or
  a `msg` verbatim, because the outer terminal shows our records as ours.
  Re-emission is coalesced to at most four reports a second.
- **Feature queries** from a pane's program are answered by our emulator, not
  passed to the outer terminal; our host supports the protocol regardless of
  what the user's own terminal does.
- **Depth.** A record whose ID plus the prefix would pass eight levels or
  128 bytes is folded into its deepest allowed ancestor, which takes the
  most urgent state among the folded records.

### Rate limits

- An emitter sends at most four reports a second per record, except a
  transition into or out of `blocked`, `done`, or `error`, which goes at once.
- The host coalesces status frames per attachment to one every 250 ms,
  carrying the latest version of each changed record, so a program that
  reports progress in a loop costs bounded relay traffic. A transition is
  never coalesced away.
- A terminal that sends more than 64 reports a second has its `progress`-only
  updates dropped for the rest of that second.

### Trust boundary

- A record is untrusted text from whatever wrote to the PTY. A `cat` of a
  log that contains the sequence sets a record; that is inherent to an
  in-band protocol, and the lifetime rules clear most of it at the next
  prompt.
- It grants nothing. No record starts, approves, answers, or closes
  anything. Our own approvals stay on their typed channels: Coder V1's
  approval events, studio proposals, and NIP-TERM proposals with their exact
  revisions.
- Every displayed record carries its origin from our side: host, pane, and
  terminal, drawn by us, never taken from `title` or `app`.
- **Who sees what.** A device with the `terminal` right, or `observe` under
  the host's observer policy, sees a terminal's records. A `watch` or
  `drive` share sees only records last updated at or after its `from`, and
  after its pause ends; a paused share sees none. Both share modes see the
  same records, and only a `drive` holder that is the typist can act on the
  pane a `blocked` record points at. Status is terminal data under
  NIP-TERM's privacy rules: never in presence, never in the viewers list,
  never in world traffic.

## Decision 4: the NIP-TERM extension

Add `openagents.terminal-status.v1`, presence capability `term-status`,
following the effects feature's pattern. The details below are the shape
for the NIP-TERM change; the fixtures fix the exact bytes.

- **Attach.** An attach may name the feature. That attachment receives
  `{type: "status", after, records, complete}` frames: first the whole
  table (paged when it passes 12,288 bytes of JSON, `complete` true on the
  last page), then one frame per coalesced change. A change carries each
  changed record whole, or `{id, removed: true}`.
- **Record.** `{id, state, kind, progress, app, title, msg, updated, seq}`:
  `id` is the full path (empty for the root), `title` and `msg` are decoded
  UTF-8 already checked by the host, `app` is the effective value after
  inheritance, `updated` is Unix milliseconds, and `seq` is the output
  frame's sequence number that set it, so a share's `from` can filter it.
- **Not replayed.** Like effect frames, status frames are unsequenced; a
  reattach gets the whole table again.
- **List read.** `openagents.terminal-status-list.v1` returns, for every
  running terminal the sender may read, its terminal reference and its
  `blocked`, `error`, and `done` records, newest first, bounded to 12,288
  bytes with a `more` cursor. This is the inbox read; it needs no attachment.
- **Snapshot.** With the feature in `requires`, the snapshot stream carries
  the `STATUS` record described above.
- **Fixtures** in `crates/coder-pty/fixtures/nip-term-ext.json`, checked by
  `tests/ext.rs`: each frame and request round-tripped exactly; a paged
  table; a share whose `from` hides an older record; a paused share that
  gets nothing; an older host refusing the feature as `unsupported_feature`;
  and a base-profile parser refusing a status frame.

## Rollout

Agent-hour estimates are at this repository's pace, as in
[many agents on one machine](../coder/design/many-agents-one-machine.md).

| Phase | Work | Estimate |
| --- | --- | --- |
| 1. Parser and conformance | `crates/program-status`: types, parser, encoder, table, detection, display sanitizing. Fixtures from every example in the spec, verbatim, plus each limit at its bound and one byte over, each error rule, the lifetime rules, `clear` of a subtree, `app` inheritance, eviction, and the security cases (C0 and C1 in decoded text, bidirectional overrides, invalid base64). | 4 |
| 2. Emulator and host | `coder-vt` parsing, table, query reply, lifetime from OSC 133 `A` and exit, the `STATUS` snapshot record; `Authority` owns the host's tables. | 4 |
| 3. Emitters | Coder V1 (3), the older `coder` and `openagents terminal` (2), `microcoder` (1), `openagents lease` (1), soak scripts and `retail-qualify` (1). Studio seats and Alice follow from Coder V1. | 8 |
| 4. NIP-TERM | The feature, wire types in `coder_pty::ext`, fixtures, `coder-host` serving and advertising `term-status`, the list read, the NIP-TERM text. | 6 |
| 5. Consumers | `terminal-core` glyphs, counts, and inbox (4); phone and browser inbox through the list read (4); Verse nameplates and boards for non-Coder panes (3); `terminal-mux` re-emission (2); orchestrator preference order (2). | 15 |
| 6. Interop and upstream | Run our emitters under Ghostty and Rex and our terminal under their example programs; a cross-check that our encoder's bytes parse in libghostty; file the spec questions below. | 3 |

Total: about 40 agent-hours. Phases 1 and 2 come first; 3 and 4 can run in
parallel after them, and 5 needs 4.

Each phase's check is `cargo test -p` for the crates it touches. The
conformance fixtures live in `crates/program-status/fixtures/` with the
spec revision they were written against; a new revision gets a new fixture
file, and the old one stays.

### Contributing back

- **Interoperate, don't fork.** We implement the published wire exactly,
  including the parts we would design differently, and keep our additions
  (the NIP-TERM feature, ASCII glyphs, the inbox) on our side of the
  terminal.
- **Report spec issues** on the spec's own channel once we have fixtures
  that show them:
  1. No multiplexer guidance: how to nest, prefix IDs, answer or pass
     through queries, and budget the eight levels.
  2. "The process attached to the terminal exits" is undefined over SSH and
     in a multiplexer, where the terminal sees the remote shell's prompt but
     not the program's exit.
  3. Plain LRU eviction can evict a `blocked` record, the one record a
     person most needs to see.
  4. No way to say a person has seen a `done` record, so every terminal
     invents its own hiding rule.
  5. No version in the sequence or the reply; a program cannot tell which
     revision a terminal implements.

## Open questions for the owner

1. **Default on?** Should Coder V1 and `openagents terminal` emit by default
   when the outer terminal answers the query, or only with
   `OPENAGENTS_PROGRAM_STATUS=1` until Ghostty ships its implementation?
   Recommended: on by default, since nothing is emitted without a reply.
2. **Phone wake.** The phone app is not bound by the terminal's
   no-notification rule. Should a `blocked` record on a host wake the phone
   through the push gateway, or only show in its inbox when opened?
3. **World visibility.** Should other players in Verse see a coarse state
   (working, blocked) on the owner's agents' nameplates, or nothing? This
   page assumes nothing.
4. **F18.** Is a new function key right for the inbox, or should it share
   F8 (panes) as a second page?
5. **Hiding across devices.** Should hiding a `done` row on one device hide
   it on all of the owner's devices? That needs a small host record; this
   page keeps it per device.
6. **Third-party records in our orchestration.** When a seat drives Claude
   Code's TUI and its record says `blocked`, should the studio raise a
   decision for the owner, or only show it? This page only shows it.
