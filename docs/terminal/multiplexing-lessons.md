# Multiplexing lessons for OpenAgents terminals

Status: design notes, October 8, 2026. Nothing here is implemented by this
page, and no row changes a contract until its owner lands it with tests.
This page reads the current terminal plan beside general lessons about
terminal multiplexers that keep sessions on a server and let many clients
attach. It says what to adopt, adapt, or skip, and where each idea lands.

Read with:

- [NIP-TERM](../../nips/openagents/NIP-TERM.md): the host terminal protocol,
  its extensions (snapshots, blocks, session records, effects, typist,
  shares), and its conformance list.
- [Smart terminal](smart-terminal.md): the multiplexer and surfaces sections,
  and its prior-art section.
- [Workbench roadmap](workbench-roadmap.md#sessions-and-the-multiplexer-for-all-work):
  the workbench session as the unit of work.
- [In-world terminal](../verse/in-world-terminal.md#multiplexing): panes,
  layout, prefix keys, and grants.
- [Coder Cloud](../cloud/coder-cloud.md#terminals-and-workbench): the browser
  terminal rules.
- The earlier Coder repository's multiplexer comparison and roadmap
  (`~/work/coder/docs/os/README.md` and issues #855 through #865 there), and
  its rendering audit
  (`~/work/coder/docs/game/2026-09-16-terminal-rendering-audit.md`) with the
  measured emulation budget.

Vocabulary follows the [glossary](../glossary.md): **host**, **device**,
**device grant**, **host generation**, **terminal session** (a host-owned
PTY), **attachment**, **typist**, **share**, **workbench session**, **pane**,
**direct channel**, **relay**, **cloud computer**. A *pane* is a viewer's
rectangle on one resource; a *workbench session* references terminals and
other resources and owns none of them.

## What we already have

The base is sound. NIP-TERM already gives:

| Property | Where |
| --- | --- |
| The host owns the PTY; a client closing ends only its attachment | NIP-TERM [Lifetime](../../nips/openagents/NIP-TERM.md#lifetime), `coder-pty` |
| Sequenced output, bounded replay, explicit `gap`, never a silent join | NIP-TERM [Frames](../../nips/openagents/NIP-TERM.md#frames) |
| Host restart is visible: earlier references refuse as `lost` | Host generation, NIP-TERM |
| A parsed-state snapshot on join with parser continuation and history pages | NIP-TERM [Attach by snapshot](../../nips/openagents/NIP-TERM.md#attach-by-snapshot), `coder_vt::Authority` |
| One owner for side effects (query replies, bell, title, clipboard) | NIP-TERM [Effects](../../nips/openagents/NIP-TERM.md#effects) |
| One typist; viewers draw at the typist's size and pan | NIP-TERM [Typist](../../nips/openagents/NIP-TERM.md#typist) |
| Per-terminal watch and drive shares, narrowing only, expiring | NIP-TERM [Shares](../../nips/openagents/NIP-TERM.md#shares) |
| Session records with layout and exact-revision writes | NIP-TERM [Sessions](../../nips/openagents/NIP-TERM.md#sessions) |
| Rights rechecked per operation; input never queued offline | NIP-TERM [Rights](../../nips/openagents/NIP-TERM.md#rights) |

The lessons below are about what happens around that base: reconnects,
many panes, slow clients, scripting, and the browser and phone.

## Design principles

1. **The host is the only authority.** It owns processes, the parse, side
   effects, the typist role, and the layout revision. Clients hold caches
   and viewer-local state (scroll, selection, font, keymap mode).
2. **Two planes.** Requests, results, and events about structure are small
   and ordered. Terminal bytes are large and per terminal. Keep them on
   separate logical channels so a busy terminal never delays a layout
   change, a typist change, or a revocation.
3. **Every reconnect is a reconciliation.** A client never assumes it
   resumed. It proves the host generation, its applied sequence, its layout
   revision, and its role, and the host answers with what still holds.
4. **Durability is stated, never implied.** A client closing survives. A
   reattach survives through the snapshot. A host restart does not; panes
   show `lost`. A snapshot is a view, never a process checkpoint.
5. **Bounded everything, refused not truncated.** Message size, queue
   depth, snapshot size, history pages, sessions per owner, attachments
   per terminal. Over a bound, refuse with a code the client can act on.
6. **The transport grants nothing.** A private network, a loopback socket,
   or a tunnel is a route, not an identity. Rights come from device grants
   and shares, checked per operation.
7. **Everything a human can do, a script can do, under the same rights.**
   One operation set serves the window, the phone, the browser, the TTY
   client, the CLI, and agents.

## Recommendations

Each row is marked **adopt** (take as is), **adapt** (take the idea in our
shape), **have** (already true; keep it), or **skip**, with the reason.

### Protocol and state model

| Lesson | Mark | Where it lands | Reason |
| --- | --- | --- | --- |
| Separate the structure plane from the byte plane: one ordered request/event stream per connection, and a byte stream per attached terminal that can resume on its own | adapt | NIP-TERM [Transport](../../nips/openagents/NIP-TERM.md#transport); NIP-REACH stream multiplexing | Today output frames and control results share one ordered channel. A per-attachment logical stream on the direct channel lets the host prioritize results, typist frames, and `detached` over bulk output. Over a relay, mailboxes already separate them. |
| Binary output frames on a direct channel instead of base64 JSON | adapt, measure first | A new NIP-TERM extension (binary output), `coder-pty::wire` | Base64 adds a third to every output byte. Worth it only if the [performance workload](../verse/verification/2026-10-05-terminal-performance/README.md) shows the encode or the bandwidth matters on phones. Keep JSON on relays. |
| Layout as a revisioned document with small, validated mutations (split, move, swap, zoom, resize, focus) instead of whole-record replacement | adapt | NIP-TERM [Sessions](../../nips/openagents/NIP-TERM.md#sessions), `terminal-core::layout` | Whole-record writes with `base` work for one editor. Two devices rearranging panes collide on every write. Typed mutations against the current revision rebase cleanly; the host still refuses a mutation whose target is gone. |
| Push a content-free `session_changed` event (session ID and new revision) to attached clients | adopt | NIP-TERM sessions extension, `coder-host` | Clients poll today. An event with no names or layout keeps the privacy rule and lets every device follow a layout change at once. |
| Clients apply their own layout mutations optimistically, keyed by a client sequence, and reconcile against the acknowledged revision | adapt | `terminal-core`, `terminal-remote` | Pane operations must feel local. The host's revision stays authoritative; a refused mutation rolls back visibly. |
| Tag every asynchronous reply with the connection's generation and drop replies from an earlier connection | adopt | `terminal-remote`, `coder_computers::live` | A reply that arrives after a reconnect (keymap, session read, history page) must never apply to the new state. Line epochs and `stale` already do this inside NIP-TERM; apply it to every client cache. |
| A self-description operation: the operations, features, bounds, and schemas this host serves | adapt | Host presence capabilities plus a NIP-TERM `describe` result; `openagents computer shell --describe` | Agents and scripts should discover what a host serves without trial and refusal. Presence lists features; it does not list bounds. Keep it content-free. |
| Resolve an omitted target deterministically: the one live session, else `ambiguous` with the candidates, else `not_found` | adopt | `openagents` CLI, `terminal-mux` | A CLI that guesses which terminal to type into is unsafe. This is bounded parsing of IDs after a command was chosen, not intent routing. |
| Keymap modes live in the client; the host keeps no mode state | have | `terminal-core::keys`, `terminal-mux` prefix | Matches today's `Ctrl+B` prefix. Keep it. |

### Persistence and reattach

| Lesson | Mark | Where it lands | Reason |
| --- | --- | --- | --- |
| A resumable attachment: on transport loss the host keeps the attachment, its position, and its typist role for a short grace period; the same principal resumes it with an attachment-bound secret issued in the `attached` result | adopt | NIP-TERM attach and typist sections; `coder-pty` host, `terminal-remote` | Today a closed transport ends the attachment and the typist role, so a phone that switches from Wi-Fi to cellular loses the keys mid-command. Resume within grace keeps the role; after grace, today's rules apply. The secret is not a bearer credential: the host still checks the principal's grant. |
| Resume only after a loss, never to fork a second reader; one consumer per attachment | adopt | Same | A resumed attachment must not be readable twice. A second resume with the same secret refuses. |
| Reconnect proactively on network-path change and on system wake, and record each resume step | adopt | `terminal-remote`, the phone's `coder_computers::live`, `coder-connect` | The client knows before the transport times out. A small resume log (generation proven, sequence applied, role kept or lost) makes field failures diagnosable without terminal content. |
| Detect that a different host process answered: compare host key and generation before reusing any cached reference | have | Host generation, `lost` | Already the contract. Clients must also discard cached snapshots and history on a generation change. |
| Restore a workbench session after a host restart by layout only: terminals read `lost`, a new shell is offered, never presented as the old one | have | NIP-TERM sessions | Keep it. |
| Seal a dormant terminal's parsed state to disk to free memory, and resume it empty with a clear notice if the file is unreadable | skip for now | `coder-vt`, `coder-pty` | NIP-TERM says the host never writes terminal data to disk. Revisit only if host memory under many terminals forces it, and then with encryption at rest, retention equal to the in-memory ring, deletion on close, and a NIP-TERM privacy amendment. |
| Compress idle scrollback in memory | adapt, later | `coder-vt` | The earlier Coder repository halved an idle 20,000-line block this way. Do it when a measured host keeps many terminals, not before. |

### Multiple clients and remote observation

| Lesson | Mark | Where it lands | Reason |
| --- | --- | --- | --- |
| Clients ask only for what they show: the layout and pane labels, output for visible panes, and interaction for the focused one | adopt | `terminal-core`, `terminal-remote`, web workbench | A phone listing twelve panes should not stream twelve terminals. Hidden tabs detach or hold a low `rate`; a visible pane attaches by snapshot. The host already narrows `rate`; the client should ask for less. |
| Each attached client has a kind: window, phone, browser, TTY, CLI, or agent | adopt | NIP-TERM `viewers` result, typist frames | The viewers list and the "who is typing" label should say whether a person or an agent is attached. The kind is declared by the client and is display only; rights come from grants. |
| Agents attach under the same rules as people, visibly, and type only under a handoff | have | NIP-TERM [agent handoff](../../nips/openagents/NIP-TERM.md#typist) | Keep it. |
| Programs inside a pane can call the multiplexer (open a split, send keys elsewhere) | adapt | `coder-host` launch environment, `openagents` CLI | Useful for scripts and agents. A process in a pane holds no grant, so such calls become requests the typist approves, as shell proposals are, never silent authority. |
| Observation over a relay at a lower rate, refused above the relay ceiling | have | NIP-TERM transport | Keep it. Phones on cellular use it. |

### Permissions and grants

| Lesson | Mark | Where it lands | Reason |
| --- | --- | --- | --- |
| Treat a private network or tunnel as sufficient authentication | skip | n/a | It would make every tailnet member or tunnel user a typist. Every operation keeps device-key authentication and a grant check. |
| Record the local peer's operating-system identity on the loopback socket and refuse a different user | adopt | `coder-host` control socket | The loopback socket is a route; checking the peer's user ID stops another local account from using it. The principal remains the device key. |
| Refuse configurations that would admit nobody or everybody, with a sentence that says what to change | adopt | `coder-host` enrollment and presence | An owner-only policy plus a machine identity with no owner admits no one. Say so at configuration time, not as a silent failure later. |
| Recheck rights on every operation and end attachments on revocation | have | NIP-TERM rights | Keep it. |

### Rendering and resize

| Lesson | Mark | Where it lands | Reason |
| --- | --- | --- | --- |
| One authoritative parse on the host, one emulator per client, raw bytes to both | have | `coder_vt::Authority`, `coder-vt` | Keep it. |
| The typist's client reports its palette so the host answers color queries (OSC 10, 11, 4) with the colors the typist sees | adopt | NIP-TERM effects extension, `coder_vt::Authority` | The host answers queries now, but it does not know the typist's theme. A program that asks for the background color should get the typist's. Viewers keep their own palette locally. |
| Make sure the host's `TERM` has a matching terminfo entry on that host | adopt | `coder-host` launch environment, `coder-ssh` install | A remote host without the entry breaks full-screen programs. Install the entry with the host, or fall back to a widely installed value. |
| Resize: only the typist's size reaches the PTY; viewers pan around the cursor | have | NIP-TERM typist | Keep it. Coalesce a typist's resize bursts on the client before sending. |
| Advisory program status from output patterns | adapt, fallback only | [OSC 7501 proposal](program-status-osc7501.md) | Prefer explicit OSC 7501 from our programs. Pattern detection may label a pane for display only, never decide authority, routing, or billing. |

### Backpressure and scrollback

| Lesson | Mark | Where it lands | Reason |
| --- | --- | --- | --- |
| A bounded queue per attachment; on overflow, drop the queue and send a fresh snapshot instead of a long replay | have, extend | NIP-TERM snapshot join, `terminal-remote` projection queue | Hosts already replace a `gap` with a fresh snapshot for snapshot joiners. The earlier Coder repository kept 256 frames per client and sent a snapshot on overflow. Name the bound in the self-description. |
| Never block the PTY on a reader | have | NIP-TERM frames | Keep it. |
| Refuse a snapshot that would exceed its bound and let the client fall back to replay | have | NIP-TERM 4 MiB rule | Keep it. |
| Close a connection whose single event exceeds the negotiated size, with a reason, rather than splitting it silently | adopt | NIP-REACH data frames, `coder-host` | Today bounds are fixed per message type. A clear close reason helps a client lower its request instead of retrying the same one. |
| History pages on demand, newest first, sized before they arrive | have | NIP-TERM history | Keep it. |

### Failure and recovery

| Lesson | Mark | Where it lands | Reason |
| --- | --- | --- | --- |
| Every failure class has a name the client can act on: unreachable, rejected, incompatible, a different host answered, stale, lost | have, extend | NIP-TERM refusals, the glossary's *blocked connection* | Add the resume outcomes: resumed, grace expired, role lost. |
| One host process per host root, enforced by a lock; a second start reports the running one | have | Host service, host generation | Keep it. |
| A host on a cloud computer that may exit when it holds no terminals and no attachments, and reports that policy in its status | adapt | [Managed computers](../cloud/managed-computers.md), `coder-host` presence | Idle wake and teardown for cloud computers need the host to say whether it will stay up. Owned computers keep the resident service. Metering follows the cloud computer's lifecycle, not the terminal's. |
| An end of a terminal records why: exited, closed, idle expiry, host shutdown | have | NIP-TERM `exit` | Keep it. |

### Testing

| Lesson | Mark | Where it lands | Reason |
| --- | --- | --- | --- |
| An in-process transport that runs host and client in one test without sockets | adopt | `coder-pty` tests, `terminal-remote` tests | Makes resume, grace, and reorder tests fast and deterministic. |
| Fault injection: drop, duplicate, reorder, and delay frames; kill the transport mid-snapshot; change networks mid-command | adopt | NIP-TERM conformance list | Covers the resumable attachment and the snapshot boundary. |
| Two clients mutate one layout concurrently; one wins, the other rebases or refuses | adopt | Sessions conformance | Needed once layout mutations replace whole-record writes. |
| A latency budget with ten streaming terminals: PTY read to published frame, key to PTY write | adopt | [Performance workload](../verse/verification/2026-10-05-terminal-performance/README.md) | The earlier Coder repository held every median under 0.6 ms and every 99th percentile under 2.1 ms on a Mac and `coderos-4080`. Use the same harness shape for the host. |
| Fixtures that pin every new body and refusal | have | `crates/coder-pty/fixtures/` | Keep it for every new extension. |

## Where each surface stands

| Surface | Today | What these lessons change |
| --- | --- | --- |
| Native window and Verse overlay (`terminal-core`, `terminal-gfx`) | Panes attach to host terminals; layout is client state | Layout mutations against the host revision, visible-pane attach, resume within grace |
| Phone (`coder-computers::terminal`) | Snapshot join, effects, typist, reattach after a lost frame | Resume within grace on network change and wake; a layout-only mode for pane lists |
| Browser (`coder-browser`, the web workbench) | Granted host transport and shared rendering | A one-time data token minted over the authenticated connection, never a credential in a URL; layout-only listing; page close detaches |
| TTY (`terminal-mux`) | Up to eight attachments, two visible panes | Detach hidden tabs; deterministic target resolution for CLI verbs |
| Cloud computers | Retail v1 offers no customer terminal | When terminals arrive: exit-when-idle policy in presence, the same grants, no transport trust |
| CLI and agents | `openagents computer shell` | Scriptable verbs (send keys, capture the screen as text, wait for a block to finish) under the same rights, plus `--describe` |

## Top recommendations

1. **Resumable attachments.** Keep an attachment, its position, and its
   typist role through a short transport loss, resumed by the same principal
   with an attachment-bound secret. This is the largest gap for phones.
2. **Separate the structure plane from the byte plane** on direct channels,
   so results, typist frames, and revocations never wait behind output.
3. **Layout as typed mutations with pushed revisions**, replacing
   whole-record writes, so several devices can arrange one workbench
   session live.
4. **Visibility-driven attachment**: clients attach only visible panes and ask
   for the layout only otherwise, on every surface.
5. **A fault-injecting conformance suite** with an in-process transport:
   reorder, drop, duplicate, mid-snapshot loss, network change, concurrent
   layout edits, and the ten-terminal latency budget.

## Not adopted

- Trusting the network or tunnel instead of authenticating each device.
- Writing terminal content to disk, until a measured need and a NIP-TERM
  privacy amendment exist.
- Any promise that a terminal survives a host restart.
- Pattern-matched program status as anything more than a display label.
- Running out-of-process pane plugins inside the host. Pane kinds open
  resources their domain owners already serve, as the
  [workbench roadmap](workbench-roadmap.md) requires; plugins stay Wasm
  under `crates/plugin`.
