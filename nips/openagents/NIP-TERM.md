# NIP-TERM — Terminal Sessions

`draft` `optional` — v1, 2026-09-26. The [shared contracts](contracts.md)
are normative. This profile lets an enrolled device open a terminal on a
host, type into it, resize it, detach, and reattach from another device to
see the output it missed, bounded and in order. It introduces no new event
kinds.

A terminal is a host-owned pseudo-terminal (PTY) and the process tree running
on it. It outlives the clients that read it. Output carries a sequence number
per terminal; the host keeps a bounded replay buffer and says so explicitly
when a reader asks for output it discarded. Input and output are private and
never appear in a public event.

## Relationship to the existing contracts

| Existing contract | Reuse and boundary |
| --- | --- |
| [Shared contracts](contracts.md) | Encoding, common IDs, refusal codes, and the private `3188` artifact envelope. |
| NIP-HOST (drafted separately) | Host-wide device grants. NIP-TERM requires the `terminal` right it defines, and optionally honors `observe` for reading. It defines no grant of its own. |
| [NIP-REACH](NIP-REACH.md) | The authenticated direct channel. NIP-TERM bodies travel in its data frames when a direct route works. A channel grants no right. |
| [WS](NIP-WS.md) | Workspace identity. A terminal's working directory lies inside an admitted workspace root. WS states that terminal input, pointer control, and resize are live-only and that PTY framing needs its own adapter; NIP-TERM is that adapter. |
| [SESS](NIP-SESS.md), [CTRL](NIP-CTRL.md) | Engine sessions and task control. A terminal is not an engine session or a task; attaching to one grants nothing over either. |
| [LIVE](NIP-LIVE.md) | Media and scoped device input. LIVE's input profile does not cover a PTY, and NIP-TERM does not cover media. |
| [ENV](NIP-ENV.md) | Environment leases. A terminal runs on the host it was opened on; it does not allocate or attach an environment. |

## Roles and encoding

The **host** owns terminals and issues every grant that reaches them. A
**device** is an enrolled client key. The **principal** of a request is the
device key that authenticated it: the verified signer of a `3188` envelope,
or the device key a NIP-REACH channel proved.

Every body has `v`, `requires`, and exactly the fields listed; unknown fields,
versions, required features, and enum values refuse. The base profile's
`requires` list is empty; the [extensions](#extensions) define three feature
IDs, and a body that names one carries the fields that feature adds. IDs are
common IDs (64 lowercase hexadecimal characters).
Terminal input and output bytes are base64 with the standard alphabet and
padding. Strings are bounded as stated; a host refuses rather than truncates.

A **terminal reference** is `{generation, terminal}`: the host generation
(from NIP-REACH presence and handshake) and the terminal ID the host minted.
A host takes a new generation each time it starts.

## Rights

| Operation | Requires |
| --- | --- |
| Open, input, resize, signal, close | `terminal` |
| Attach in `interact` mode | `terminal` |
| Attach in `observe` mode | `terminal`, or `observe` when the host's policy lets observers read terminals |
| Detach | The principal that attached |

The host checks the current right on every operation. It also rechecks every
attachment's right periodically and ends an attachment whose right was
revoked with a `detached` frame whose reason is `revoked`. An attachment in
`interact` mode is not a standing input grant: each input, resize, and signal
is checked on its own. Output that a revoked device already received cannot
be recalled.

## Operations

Every request has `request`, a common ID the client chooses. A host applies
a request ID once per principal: an exact retry returns the original value
with status `duplicate`, and the same ID with different content refuses as
`idempotency_conflict`. The host remembers a bounded number of recent
requests; a retry older than that window is a new request.

### Open

`openagents.terminal-open.v1`:

| Field | Contract |
| --- | --- |
| `workspace` | Common ID of an admitted workspace. The host maps it to a root directory. |
| `dir` | Working directory relative to the root, at most 1,024 bytes. Empty is the root. Absolute paths and `..` components refuse. |
| `launch` | `{kind: "shell"}` for the host's configured shell, or `{kind: "command", program, args}` for an exact program by absolute path (`/…`, or on a Windows host a drive path such as `C:\…`; never a network path) and at most 256 arguments of at most 4,096 bytes each. |
| `size` | `{rows, cols}`, each 1 to 1,024. |
| `env` | At most 64 `{name, value}` entries. Names are uppercase letters, digits, and underscores; values are at most 4,096 bytes. |

The host resolves the working directory, following symbolic links, and
refuses as `not_admitted` when the result leaves the workspace root. It
resolves no program name through a search path, and refuses as
`malformed` a program that is not an absolute path on its own platform.
It clears its own
environment and sets a host-chosen base (for example `PATH`, `HOME`, and
`TERM`) plus the requested variables; a requested name outside the host's
allowlist refuses the whole request as `not_admitted` rather than being
dropped silently. The client never chooses which shell a `shell` launch runs.

### Attach and detach

`openagents.terminal-attach.v1` has `terminal` (a terminal reference),
`mode` (`interact` or `observe`), `after` (the last sequence number the
client already has; zero asks for everything retained), and `rate` (the most
output bytes per second the client wants, positive). The host narrows `rate`
to its own ceiling and never widens it. An `after` greater than the newest
sequence number refuses as `malformed`.

The host then delivers the attachment's frames, in order, beginning after
`after`. The result value reports the attachment ID, the newest sequence
number when the attachment began (`head`), the current size, and whether the
process is still running.

`openagents.terminal-detach.v1` has `terminal` and `attachment`. The host
sends a `detached` frame with reason `requested` and stops delivery. The
terminal keeps running.

### Input, resize, signal, and close

- `openagents.terminal-input.v1` has `terminal` and `data`, 1 to 4,096 bytes.
  The host writes the bytes to the PTY and returns how many it took; fewer
  than sent means the process stopped reading input. Input is live only: a
  client never queues it in an offline outbox for later delivery, and a host
  never replays it.
- `openagents.terminal-resize.v1` has `terminal` and `size`. The PTY's size
  changes and the foreground process group receives `SIGWINCH`.
- `openagents.terminal-signal.v1` has `terminal` and `signal`: `interrupt`,
  `quit`, `terminate`, `hangup`, or `kill`. The host sends it to the
  terminal's foreground process group.
- `openagents.terminal-close.v1` has `terminal`. The host ends the terminal's
  process tree as described under [Lifetime](#lifetime).

Input, resize, and signal on a terminal whose process ended refuse as
`closed`.

### Results

`openagents.terminal-result.v1` has `request` (the ID it answers), `status`
(`accepted`, `duplicate`, or `refused`), `reason` (a refusal code, non-null
exactly when refused), and `value` (null exactly when refused). A value has a
`kind`:

| Kind | Fields | Returned by |
| --- | --- | --- |
| `opened` | `terminal`, `size` | Open |
| `attached` | `attachment`, `head`, `size`, `running` | Attach |
| `written` | `bytes` | Input |
| `done` | none | Detach, resize, signal, close |

Refusal codes are the shared `malformed`, `unsupported_version`,
`unsupported_feature`, `not_admitted`, `unavailable`, `limit_exceeded`,
`idempotency_conflict`, and `revoked`, plus two NIP-TERM causes:

- `lost`: the reference names an earlier host generation. The host restarted
  and the terminal did not survive it.
- `closed`: the terminal ended and the host no longer retains it, or its
  process ended and the operation needs a running one.

An unknown terminal in the current generation refuses as `unavailable`. A
host that has no PTY implementation refuses every open as `unavailable`.

## Frames

`openagents.terminal-frame.v1` has `terminal`, `attachment`, and `body`. A
body has a `type`:

| Type | Fields | Sequenced |
| --- | --- | --- |
| `output` | `seq`, `data` (at most 8,192 bytes before base64) | Yes |
| `exit` | `seq`, `exit` | Yes |
| `gap` | `from`, `to`, `bytes` | No |
| `detached` | `reason`: `requested`, `revoked`, or `transport` | No |

Sequence numbers are per terminal. The first sequenced frame is 1, and each
later one is one more than the one before. `exit` is the terminal's last
sequenced frame and follows all output its process wrote. Its `exit` object
has `cause` (`exited`, `closed`, `idle_expired`, or `host_shutdown`), `code`
(the exit code or null), and `signal` (the signal number that ended the
process or null).

The host retains sequenced frames in a bounded replay buffer and discards
the oldest first. When a reader needs frames the buffer no longer holds, the
host sends a `gap` frame naming the discarded range `from` through `to` and
the output `bytes` in it, or null when the host no longer knows the count,
followed by the frames it still has. A host never joins the output before and
after a gap as if it were continuous.

A host delivers each attachment's frames in order and never blocks a
terminal's process on a slow reader. When an attachment's transport is
backed up or its byte budget is spent, the host holds its position and
resumes from the buffer later; if the buffer moved past that position in the
meantime, the attachment receives a `gap`. An attachment that received
`exit` is complete; the host sends it nothing further.

### Client state

A client tracks the highest sequence number through which it has applied
every frame or received a `gap`. It ignores a frame at or below that number
as a duplicate. A frame above the next expected number, without a `gap`
before it, means the transport lost frames: the client applies nothing
further and reattaches with `after` set to its applied number, and the host
either replays the missing frames or reports a `gap`. A client shows gaps as
gaps, and it treats frame content as untrusted terminal data, never as
instructions.

## Lifetime

A terminal survives client disconnection. Detaching, closing a transport, or
losing a device ends only that attachment. The terminal ends when:

- Its process exits (`exited`).
- A client with `terminal` closes it (`closed`).
- No client has been attached or typed for the host's idle period
  (`idle_expired`). The idle period starts at the last attach, detach, or
  input while no attachment remains.
- The host shuts down (`host_shutdown`).

A host ends a terminal's process tree as one unit. The terminal's process
leads its own session and process group with the PTY as its controlling
terminal. Ending sends the group `SIGHUP` and `SIGTERM`, waits a short grace
period, sends `SIGKILL`, and reaps the direct child before it records `exit`.
When the direct child exits on its own, anything left in its group is killed.
A descendant that leaves the group by starting a new session escapes this
bound, and hosts document that limit.

After a terminal ends, the host keeps its buffer for replay until the idle
period passes with no attachment, then answers references to it with
`closed`. A host that restarts keeps no terminal: every reference from its
earlier generation refuses as `lost`. A restarted host does not reopen a
terminal under an old ID or present a new process as the old one.

## Transport

A terminal needs one of two transports:

- **NIP-REACH direct channel.** Each NIP-TERM body is one message of the
  NIP-HOST direct-channel binding: its JSON follows a one-byte fragment flag
  in the channel's data frames. A request travels client to host; its result
  and the attachment's frames travel host to client. An 8,192-byte output
  frame encodes to about 11 KiB of JSON, so it fits one 16,384-byte data
  frame. The host rechecks the channel's grant and generation before each
  operation, as NIP-REACH requires.
- **Private `3188` artifacts over an admitted relay.** Each body is the
  inline JSON of one artifact whose `schema` is the body's `v`, sealed to the
  host (requests) or to the attached device (results and frames). A request
  and its result use the request ID as their mailbox, and an attachment's
  frames use the attachment ID. A host answers only a device it enrolled and
  only a request issued within 60 seconds of its clock. Retained frames can
  return in any order after a reconnect, so a client orders them by sequence
  number before it applies them. The relay restricts reads to author and
  recipient and excludes these events from search. Clients over a relay request a lower `rate` than over a direct
  channel, and a host can refuse attachments over a relay above its own relay
  ceiling as `limit_exceeded`.

Byte budgets apply on both transports: at most 4,096 input bytes per request,
at most 8,192 output bytes per frame, and at most the attachment's `rate`
output bytes per second. A host bounds concurrent terminals and attachments
per terminal and refuses above them as `limit_exceeded`.

## Privacy and disclosure

Terminal input and output, working directories, command lines, environment
values, and exit details are private. They never appear in a public event, a
presence record, a reachability hint, an activity summary, a URL, or a log
line. A host's process identifiers and process groups are host-local and
never appear on the wire. Notification and wakeup payloads carry no terminal
content. A relay sees only the envelope's visible tags, sizes, and timing,
which reveal that a device and a host exchange traffic.

## Worked flow

1. A phone with `terminal` opens a shell in a workspace, 24 rows by 80
   columns. The host returns `opened` with the terminal reference.
2. The phone attaches in `interact` mode with `after: 0`, types `make test`
   and a newline, and receives output frames 1 through 57.
3. The phone detaches. The build continues; frames 58 through 140 accumulate
   in the host's buffer, which discards frames 58 through 90 to stay within
   its bound.
4. A laptop with the same owner's grant attaches with `after: 57`. It receives
   a `gap` from 58 to 90 with the discarded byte count, then frames 91 through
   140, then live output.
5. The build exits with code 2. Every attachment receives `exit` with cause
   `exited` and code 2 as sequence 141.
6. The host restarts. The laptop's reattach refuses as `lost`, and the laptop
   shows the terminal as lost rather than resumed.

## Extensions

Added 2026-10-05. Four optional features extend the base profile. A host
that implements one serves it; a client uses one only after the host
advertises it. Nothing here adds a right, a relay authority, or an event
kind: every extension operation needs the right its base operation needs,
travels on the same transports, and follows the same privacy rules.

| Feature ID | Presence capability | Adds |
| --- | --- | --- |
| `openagents.terminal-snapshot.v1` | `term-snapshot` | Attach by snapshot, the history operation, and record streams |
| `openagents.terminal-blocks.v1` | `term-blocks` | Paged block-journal reads |
| `openagents.terminal-sessions.v1` | `term-sessions` | Session records: membership and layout |
| `openagents.terminal-effects.v1` | `term-effects` | The host answers queries; effects arrive as effect frames |

### Negotiation and compatibility

A host lists the capability of each feature it serves among its
[REACH](NIP-REACH.md) presence capabilities, and drops the capability before
it stops serving the feature. A client names a feature in a request's
`requires` list only when the host's current presence lists its capability.
A body that names a feature carries the fields that feature adds; the same
fields without the feature refuse as `malformed`.

- **An older host.** A host that predates a feature refuses any request that
  names it as `unsupported_feature`, and refuses an extension schema it
  does not know as `unsupported_version`. A client that receives either
  refusal after presence advertised the feature treats the presence as stale
  and retries with the base profile: an attach without `requires` replays
  the ring as before.
- **An older client.** A host sends extension frames and extension result
  values only in answer to a request that named the feature. An attachment
  made without `requires` receives exactly the base profile's frames, so a
  replay-only client never sees a body it does not know.
- **Partial support.** The features are independent. A host can serve blocks
  without snapshots; a request that names a feature the host does not serve
  refuses as `unsupported_feature`, even when the host serves the others.

### New refusal causes

The extensions use three more shared codes:

- `stale`: the request names a line epoch or a session revision that is no
  longer current.
- `content_unavailable`: the requested history rows or block-journal entries
  left the host's retention.
- `identity_mismatch`: a record stream or a reference binds a different
  terminal or host generation than the frame or request that carries it.

A client that receives `stale` reads the current state again; it never
applies an answer bound to an earlier epoch or revision to the current one.

### Line epochs

A host that serves snapshots or blocks runs one authoritative emulator per
terminal and numbers every line it has produced: the first line of the
terminal is absolute line 0, and a line keeps its number as it scrolls into
history. The *line epoch* is a positive integer, 1 when the terminal opens,
that the host increases whenever absolute line numbers stop naming the same
text: a full reset (`RIS`), an erase of the scrollback (`ED 3`), or any
operation that renumbers retained lines. Every snapshot, history read, and
block line range names the epoch it belongs to. An epoch is per terminal and
per host generation.

### Attach by snapshot

With `openagents.terminal-snapshot.v1` in `requires`, an attach request
carries one more field, `join`:

- `replay`: the base behavior; the host replays the ring after `after`.
- `snapshot`: `after` must be 0. Instead of replaying the ring, the host
  sends a snapshot of its emulator's parsed state as a record stream, and
  then the live frames that follow it.

A host whose snapshot prefix (every record through `READY`) would exceed
4 MiB refuses the attach as `limit_exceeded`; the client attaches again with
`join: "replay"`. The result value is the base `attached` value.

### Record streams

A *record stream* is an ordered byte sequence of records. A host sends it to
one attachment in records frames, `openagents.terminal-records.v1`:

| Field | Contract |
| --- | --- |
| `terminal` | The terminal reference. |
| `attachment` | The attachment the stream is for. |
| `stream` | A common ID the host minted for this stream. |
| `part` | The part's index, 0 for the first, one more for each later part. |
| `last` | Whether this is the stream's final part. |
| `data` | 1 to 8,192 bytes of the stream, base64. A record may span parts. |

Records frames are not sequenced and take no sequence number. A client
concatenates the parts of a stream in `part` order. Over a relay, parts can
arrive out of order: a client holds at most 64 parts ahead of the next
expected one and discards the stream when it would need more. A part index
repeated with the same bytes is a duplicate and is ignored; with different
bytes, or a part after `last`, the stream is malformed. A stream is at most
16 MiB.

Each record is a 10-byte header followed by its payload, all little-endian:

| Bytes | Field |
| --- | --- |
| 0 to 1 | `tag`, an unsigned 16-bit record type |
| 2 to 5 | `length`, an unsigned 32-bit payload length, at most 1,048,576 |
| 6 to 9 | `crc32c`, the CRC-32C (Castagnoli) checksum of the payload |
| 10 on | The payload |

This is the record shape of libghostty's Snapshot v1 (tag, length, CRC-32C),
reimplemented; the tags and payloads are this profile's own and do not
interoperate with libghostty. A length above the bound, a length that runs
past the end of a complete stream, a checksum that does not match, an
unknown tag, or a record out of order makes the whole stream malformed.

| Tag | Record | Payload |
| --- | --- | --- |
| 1 | `TERMINAL` | JSON object: the stream's binding |
| 2 | `STATE` | JSON object: cursor, pen, modes, and other screen state |
| 3 | `ROWS` | JSON object: a page of one screen's rows |
| 4 | `CONTINUATION` | Raw bytes, at most 4,096: input the parser holds unfinished |
| 5 | `READY` | Empty |
| 6 | `HISTORY` | JSON object: a page of history rows |
| 7 | `FINISH` | JSON object: what the stream sent |

JSON payloads are compact UTF-8 objects with exactly the fields below; an
unknown field refuses as `malformed`. Text in a payload is terminal data
with every control character removed; a payload that contains one (U+0000
to U+001F, U+007F to U+009F) is malformed.

A snapshot stream orders its records as `TERMINAL`, `STATE`, the `ROWS` of
the primary screen, the `ROWS` of the alternate screen when it is active,
an optional `CONTINUATION`, `READY`, zero or more `HISTORY` pages, and
`FINISH`. A history stream is `TERMINAL`, zero or more `HISTORY` pages, and
`FINISH`.

#### TERMINAL

| Field | Contract |
| --- | --- |
| `format` | 1. |
| `generation`, `terminal` | The terminal reference the stream describes. A value that differs from the records frame's `terminal` refuses as `identity_mismatch`. |
| `epoch` | The line epoch. |
| `through` | The last sequenced frame the state reflects, 0 when none. |
| `size` | `{rows, cols}`. |
| `history` | `{first, count}`: retained history is absolute lines `first` through `first + count - 1`, so screen row 0 is absolute line `first + count`. A client sizes its scroll bar from it before any page arrives. |
| `exit` | The terminal's `exit` object when the process already ended and its `exit` frame is at or below `through`, else null. |

#### STATE

| Field | Contract |
| --- | --- |
| `alternate` | Whether the alternate screen is active. |
| `cursor` | `{row, col, pending_wrap, visible, shape, blink}`; `row` and `col` lie within `size`, `shape` is `block`, `underline`, or `bar`. |
| `pen` | The style new text takes (see `ROWS`). |
| `saved_primary`, `saved_alternate` | `{row, col, pending_wrap, pen}` saved by `DECSC` for each screen, or null. |
| `scroll` | `{top, bottom}`, the scrolling region, with `top < bottom < rows`, or `0` and `0` on a one-row terminal. |
| `tabs` | Tab-stop columns, ascending, distinct, each below `cols`. |
| `modes` | `{private, ansi}`: the `DECSET` and `SM` mode numbers that are set, each list ascending and distinct, at most 64. |
| `charsets` | `{g0, g1, shift}`: `ascii` or `dec_special` for each set, and the active set, 0 or 1. |
| `keyboard` | `{primary, alternate}`: each screen's Kitty keyboard flag stack, bottom first, at most 16 entries. |
| `title` | The window title, at most 1,024 bytes. |

#### ROWS and HISTORY

`ROWS` is `{screen, first, rows}`: `screen` is `primary` or `alternate`,
`first` the screen row of the page's first row, and `rows` 1 to 64 rows.
The pages of one screen cover rows 0 through `size.rows - 1` exactly once,
in order.

`HISTORY` is `{first, rows}`: the absolute line of the page's first row and
1 to 256 rows, oldest first. Pages arrive newest first and are contiguous:
the first page ends at absolute line `history.first + history.count - 1` in
a snapshot stream, or at `before - 1` in a history stream, and each later
page ends where the previous one began. No page starts before
`history.first`.

A row is `{wrapped, runs}`: `wrapped` says the row continues on the next
line, and `runs` is a list of `{text, cells, style}`. `cells` is the number
of columns the run's text occupies (a wide character takes two), at least
1; the text is nonempty and at most 16 bytes per cell. The cells
of a row's runs total at most `size.cols`; the rest of the row is blank in
the default style. A style is `{fg, bg, flags, link}`: each color is
`"default"`, `{"index": n}` (0 to 255), or `{"rgb": [r, g, b]}`; `flags` is
a bit set of bold (1), dim (2), italic (4), underline (8), blink (16),
inverse (32), hidden (64), and strike (128), and any other bit is
malformed; `link` is the run's OSC 8 target, at most 2,048 bytes, or null.

#### CONTINUATION, READY, and FINISH

`CONTINUATION` holds the bytes since the parser was last in its ground
state: an unfinished escape sequence or a partial UTF-8 character. A client
feeds them to a fresh parser before the first live frame, so the next output
continues the sequence the host's emulator is in. The state already holds
their effects, such as a control character inside the sequence, so the client
feeds them for the parser's position only and applies nothing they do.

When the unfinished input exceeds 4,096 bytes, the host sends instead a
short prefix that leaves a fresh parser with the same effects from then on,
and brings its own parser to the matching state. Inside a control sequence
or a DCS string, the prefix makes the sequence one the parser ignores. Inside
an OSC string, the host abandons the string and drops the rest of it up to
its terminator, and the prefix opens an OSC string no handler answers.
Inside a partial UTF-8 character, both sides start a fresh parser and the
host sends no `CONTINUATION`.

`READY` means the client has everything it needs to draw and to resume
parsing. `FINISH` is `{rows, complete}`: the number of history rows the
stream sent, and whether they reach `history.first`. A snapshot stream sends
at most 2,000 history rows and at most 1 MiB of `HISTORY` records; a client
asks for older rows with the history operation.

### The snapshot and live boundary

The snapshot reflects every sequenced frame through `through` and nothing
after it. The host delivers every part up to and including the one that
completes `READY` before any sequenced frame of the attachment. After that
part it delivers sequenced frames from `through + 1` and may interleave them
with the stream's remaining parts. If the ring discards frames after
`through` before they are delivered, an attachment that joined by snapshot
receives a fresh snapshot stream in place of a `gap`: the same rules apply to
it, and its `through` is the newest frame when it was taken. An attachment
that joined by replay receives a `gap`, as in the base profile. A host that
cannot carry record streams on an attachment's transport refuses an attach by
snapshot as `unsupported_feature`.

A client applies a snapshot this way:

1. It draws nothing from the stream until `READY`. The host sends no
   sequenced frame before it, but over a relay artifacts can arrive in any
   order, so a client holds a sequenced frame that arrives first, at most
   4,096 of them, and applies it after `READY`. Past that bound the client
   discards the stream, detaches, and attaches again. On a direct channel,
   which keeps the host's order, a client may treat such a frame as a
   protocol error instead, as `coder_pty::ext::SnapshotJoin` does.
2. At `READY` it restores the screens and state, feeds the `CONTINUATION`
   bytes to its parser, and sets its applied sequence number to `through`.
   From then on the base profile's [client state](#client-state) applies.
3. It places each `HISTORY` row at its absolute line. Live output that
   scrolls rows into history takes the next absolute lines above the
   screen, so history pages and live frames never collide.
4. When a stream is malformed before `READY`, the client discards it and
   attaches again (by snapshot or by replay). When it is malformed after
   `READY`, the client keeps the screen it drew, discards the rest of the
   stream, and reads missing history with the history operation.
5. A fresh snapshot stream replaces the screen at its `READY` the same way.
   A client tells a snapshot stream from a history stream by its second
   record: `STATE` in a snapshot, `HISTORY` or `FINISH` in a history stream.
   It ignores the parts of a stream it already finished, which a relay can
   deliver again after a reconnect.

A snapshot is a view of parsed state, not a process checkpoint. It does not
survive a host restart; a reference from an earlier generation still refuses
as `lost`.

### History

`openagents.terminal-history.v1` reads older history rows into a record
stream. It has `requires: ["openagents.terminal-snapshot.v1"]`, `request`,
`terminal`, `attachment` (the principal's own attachment on that terminal),
`epoch`, `before` (an absolute line; the rows end at `before - 1`), and
`rows` (1 to 2,000). It requires the right that attachment needs.

- A current epoch other than `epoch` refuses as `stale`.
- A `before` above the newest history line plus one refuses as `malformed`.
- A `before` at or below `history.first` refuses as `content_unavailable`:
  the rows left the host's retention.
- An attachment that is not the principal's refuses as `not_admitted`.

The result value is `{kind: "stream", stream}`, and the stream (`TERMINAL`,
`HISTORY` pages, `FINISH`) follows on the attachment.

### Block journal

With `openagents.terminal-blocks.v1`, a host keeps a bounded block journal
per terminal from the shell-integration marks its emulator parses (OSC 133
and OSC 7, and the hook's private command line). The journal holds each
block's record without its output, and the output's sequence range. It lives
and ends with the terminal; a host never writes it to disk or a log.

`openagents.terminal-block-page.v1` has `requires:
["openagents.terminal-blocks.v1"]`, `request`, `terminal`, `before` (a
block number; the page holds older blocks, or null for the newest), and
`limit` (1 to 32). It requires the `terminal` right, or `observe` under the
host's observer policy. A `before` older than the oldest retained block
refuses as `content_unavailable`.

The result value is `{kind: "blocks", page}`. A page is `{newest, oldest,
blocks, more}`: the newest and oldest retained block numbers (null when the
journal is empty), at most `limit` blocks newest first, all below `before`,
and whether older retained blocks remain. The host returns fewer blocks than
`limit` to keep the result's JSON at most 12,288 bytes. A block is:

| Field | Contract |
| --- | --- |
| `block` | A positive number, increasing per terminal and never reused in a generation. |
| `origin` | `typed`, `proposal`, `agent`, or `unattributed`. A host records `unattributed` unless an attributed operation started the command. |
| `command` | The command line, at most 1,024 bytes; `command_truncated` says whether the host cut it. |
| `dir` | The working directory OSC 7 reported, at most 1,024 bytes, or empty. |
| `started`, `ended` | Unix milliseconds, or null when unknown or still running. |
| `status` | The exit status, or null. |
| `state` | `running`, `finished`, or `abandoned` (a new prompt arrived without an end mark). |
| `alternate` | Whether the command entered the alternate screen. Such a block has no output range. |
| `output` | `{from, to}`, the sequence numbers of its output, or null. |
| `retained` | Whether the ring still holds every frame of `output`. |
| `lines` | `{epoch, start, end}`, its absolute lines, or null. |

Marks are advisory: a program can print OSC 133 itself. A block record
shapes how a client draws and navigates; it never authorizes running,
sharing, or attaching anything.

### Sessions

With `openagents.terminal-sessions.v1`, a host keeps named *session*
records: which terminals and other resources belong together, and a default
layout. A session record survives host restarts; its terminals do not.

`openagents.terminal-session-read.v1` has `requires:
["openagents.terminal-sessions.v1"]`, `request`, and `session`. Its value is
`{kind: "session", record}`. `openagents.terminal-session-write.v1` has the
same `requires`, `request`, `session` (null to create a session), `base`
(the revision the write replaces, 0 to create), and `record`. A write whose
`base` is not the current revision refuses as `stale`; nothing is merged.
The value is the stored record. Both require the `terminal` right.

A record is `{session, revision, name, members, layout}`:

- `session` is the host's common ID for the session: null in a create, and
  in a write the same as the request's `session`.
  `revision` increases by one per write; a write sends 0 and the host sets
  it.
- `name` is 1 to 128 bytes without control characters.
- `members` is at most 64 entries `{member, kind, ...}`, each `member` a
  distinct number from 1. A `terminal` member has `terminal`, a terminal
  reference, and `state`, which a write sends as null and the host sets on
  every read: `live`, `closed`, or `lost` (an earlier generation). A
  `resource` member has `resource`, a JSON object of at most 2,048 bytes
  that the [workbench resource-reference contract](../../docs/terminal/workbench-resources.md)
  defines; the host stores
  it and returns it unchanged without resolving it.
- `layout` is `{tabs, active}`: 1 to 16 tabs, each `{name, root}` with a
  name of at most 64 bytes, and `active` the index of the selected tab. A
  node is `{kind: "pane", member}` or `{kind: "split", axis, ratio, first,
  second}`, where `axis` is `rows` or `columns` and `ratio` is the first
  child's share in thousandths, 1 to 999. A tree is at most 16 deep, and a
  member appears in at most one pane. A pane naming no member refuses as
  `malformed`.

A record's JSON is at most 12,288 bytes. It never holds terminal output, a
title, a working directory, a command line, or an environment value, so a
session can be listed and laid out without disclosing what ran in it. A
host bounds the number of sessions per owner and refuses above it as
`limit_exceeded`.

### Effects

A host that serves `openagents.terminal-effects.v1` runs one authoritative
emulator per terminal and parses every output byte once, before any
attachment sees it. An attach may name the feature in `requires`, alone or
with the snapshot feature; it adds no field.

- **Query replies.** The host writes the replies the program asks for, such
  as device status, cursor position, and device attributes, to the
  terminal's input. A client on an attachment that named the feature never
  sends replies from its own emulator. While any `interact` attachment that
  did not name the feature is attached, the host sends none: an older
  client still answers, and a query is never answered twice.
- **Effect frames.** The host sends an attachment that named the feature an
  `effect` frame, `{type: "effect", after, effect}`, for each effect the
  output caused. `after` is the sequence number of the output frame that
  caused it, and the host delivers the frame only after that output frame or
  a gap past it. Effect frames are not sequenced and are never replayed: a
  reattach or a replay causes no effect again. An attachment's pending
  effects hold at most one of each kind; a later title replaces an earlier
  one, and bells add up.
- **Kinds.** `effect` is one of `{kind: "bell", count}` (at least 1),
  `{kind: "title", title}` (at most 1,024 bytes, no control characters),
  `{kind: "directory", dir}` (the OSC 7 path, at most 4,096 bytes), and
  `{kind: "clipboard", text}` (an OSC 52 write, at most 8,192 bytes, with no
  control character but newline and tab). A new attachment first receives
  the current title and directory with `after` 0.
- **Clipboard.** A clipboard write reaches only the `interact` attachments
  of the principal whose input the terminal took last; the host delivers a
  longer or malformed write to no one. A client may refuse any write. A
  program can never read a clipboard: the host answers no OSC 52 query.

A client takes bells, titles, and clipboard writes from effect frames and
not from its own parsing, so a device that joins late or replays the buffer
rings no bell and writes no clipboard again.

### Extension privacy

Snapshots, history, block records, and session records are terminal data
under [Privacy and disclosure](#privacy-and-disclosure). A host builds a
snapshot or a history stream for one attachment and keeps no copy after it
is sent. Over a relay, records frames are sealed to the attached device and
use the attachment ID as their mailbox, like base frames.

### Extension conformance

The fixtures are in
[`crates/coder-pty/fixtures/nip-term-ext.json`](../../crates/coder-pty/fixtures/nip-term-ext.json),
checked by `crates/coder-pty/tests/ext.rs`. They cover:

- Every extension request and result value, round-tripped exactly.
- A complete snapshot stream in one part, the same stream fragmented across
  parts with records spanning them and parts arriving out of order, and a
  history stream.
- Streams with a corrupted checksum, a length past the bound, a length past
  the end, an unknown tag, records out of order, a binding for another
  generation, and an incomplete final part, each refused.
- A history read that left retention, a stale epoch, and a bounded block
  page.
- A base-profile checker refusing an extension attach as
  `unsupported_feature`, and a base-profile frame parser refusing a records
  frame.
- A join timeline: a sequenced frame before `READY` refused, then live
  frames after `through` applied in order.

## Implementation status

[`crates/coder-pty`](../../crates/coder-pty/README.md) implements the host
and client halves on macOS and Linux: real PTYs through `openpty`, session and
process-group ownership, the bounded replay buffer, sequence numbers, gaps,
attach and detach bookkeeping, per-attachment byte budgets, idle expiry,
host-shutdown cleanup, request deduplication, and the portable client state
with a bounded plain-text screen buffer. The host takes a rights check and a
frame sink as traits. The resident host in
[`crates/coder-host`](../../crates/coder-host/README.md) wires them to
NIP-HOST grants, NIP-REACH direct channels, and sealed `3188` artifacts, and
derives each run's terminal generation from the host key and its NIP-REACH
generation. The
Coder mobile terminal screen
([`coder-computers::terminal`](../../crates/coder-computers/README.md#terminal))
is a client: it opens with NIP-HOST `terminal.open`, attaches in `interact`
mode, orders frames, shows gaps, reattaches after a lost frame or a new
route, and reports `lost` after a host restart, drawing output with the
[`coder-vt`](../../crates/coder-vt/README.md) emulator. On
platforms without a Unix PTY the host refuses every open as `unavailable`.
The fixtures are in
[`crates/coder-pty/fixtures/nip-term.json`](../../crates/coder-pty/fixtures/nip-term.json).
The [extensions](#extensions)' wire contract and its validation are in
`coder_pty::ext`. The host serves the effects feature when it runs an
emulator (`coder_pty::host::Config::emulator`, which `coder-host` fills with
`coder_vt::Authority`), and the snapshot feature when that emulator writes
snapshots, as `coder_vt::Authority` does: attach by snapshot, history reads,
and fresh snapshots for an attachment that falls behind, over direct
channels and relays. `coder-host` advertises `term-effects` and
`term-snapshot`. The Coder mobile terminal screen joins by snapshot with
effects, restores through `coder_vt::Streams`, and asks for less when an
older host refuses a feature. No host serves the block journal or session
records yet, so a host advertises neither capability.

## Conformance

Required host and client tests:

- An echo round trip through a real PTY, and a process that sees a terminal
  on standard input and output.
- Resize observed by the process.
- Two attachments that receive identical frames.
- Detach, output while detached, and reattach from another device with
  replay that neither repeats nor skips a frame.
- Replay after the buffer wrapped, reported as a gap with its byte count.
- Input, resize, signal, close, and interactive attach without `terminal`
  refused, with nothing written to the PTY; observer reads only under policy.
- Revocation that ends an existing attachment.
- Process exit reported with its code or signal after all output.
- Close, idle expiry, and host shutdown that end the whole process group,
  including processes that ignore `SIGHUP` and `SIGTERM`.
- A reference from an earlier host generation refused as `lost`.
- Exact retries answered once, and a reused request ID with changed content
  refused.
- A working directory that escapes the workspace through a symbolic link, and
  an environment name outside the allowlist, refused.
- A frame that arrives ahead of the next expected one detected by the client.
