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
versions, required features, and enum values refuse. The initial `requires`
list is empty. IDs are common IDs (64 lowercase hexadecimal characters).
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
