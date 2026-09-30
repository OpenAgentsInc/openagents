# Desktop task chat verification

Issue: [#10016](https://github.com/OpenAgentsInc/openagents/issues/10016).
Code: `82aae1fee0` on `main`.

A bound desktop chat reads its task through the same admitted task and history
clients as a phone. The activity reader uses the phone summary builder. Its
ATIF records use the shared conversation parser and native transcript projection
for messages and tool rows. The composer shares the phone's Send, Queue, Answer,
and steering choices. Approval answers remain task input and grant no authority.

The task reader bounds its rows, projected text, raw record assembly, and queue
page. Split records assemble across pages; duplicate records retain stable row
identities. Old activity, another host or task, and obsolete history reads do
not replace current state. Source replacement refreshes cursors. A background
read does not block Enter. Mutation retries preserve their IDs and bytes, and
an acknowledgment clears only the draft that submitted those bytes. A verified
refusal leaves the draft editable. Changes to task state invalidate a pressed
transcript control before release.

## Checks

Rust 1.97.1 on an M5 Max Mac:

- Shared application: 84 tests passed, including seven task-state checks for
  exact retries, background reads, host/task/revision scope, questions,
  approvals, Stop, steering, split records, source refresh, and queue privacy.
- Desktop library: 83 tests passed, including two task-composer checks for
  current controls, exact retries, preserved newer edits, and retained drafts
  after refusal. Binary: 32 existing checks passed, four opt-in checks ignored;
  the additional task-mode capture check passed at both window sizes.
- Native adapter: 47 tests passed. Each trailing space immediately advances
  the caret and remains selectable. The open offline fixture was checked with
  individual native Space key presses, before any following letter.
- Local control wire: eight checks passed, including the unchanged closed
  phone summary contract and rejection of extra private content. Existing
  host control: five passed. Durable task-command semantics: 14 passed.
- Scratch-host acceptance: both checks passed. The real desktop socket reaches
  the real resident host and durable inbox with temporary state, keys, relay,
  and checkout. Queue, lease, edit, release, exact receipt replay, steering,
  Stop, and archive pass. Archived and malformed identities are refused.
  The earlier handoff and restart checks remain green. No inference login or
  owner state is accessed.
- Phone library: 142 passed, 19 opt-in checks ignored. Targeted formatting,
  strict all-target Clippy for affected crates, and strict Clippy for the Coder
  acceptance check passed. The optimized desktop build passed. The existing
  debug Coder link still reports its large unwind-table warning.

Native captures: [running task](running.png),
[question at minimum size](question-minimum.png), and
[approval at minimum size](approval-minimum.png). These exercise native view
validation, layout, painter, and control placement; they use synthetic task
state. The shared ATIF parser checks and scratch host checks cover transport
and record behavior separately.

The optimized foreground fixture uses 3,300 rows, 500 chats, a Grid backdrop,
1,200 × 840 points at 2×, and 115 samples per active phase. The retained
[native measurements](native.json) show p50/p99 CPU work plus submission:
scroll 2.686/4.521 ms, streaming 6.753/7.367 ms, and sidebar 5.025/5.839 ms.
Idle CPU was 3.73% of one core; peak RSS was 229.1 MiB. These measure CPU work
and submission, not GPU execution completion.

The installed OS key source, configured inference engine, and paired phone's
live question and approval run remain owner checks in `NEEDS_OWNER.md` under
#10014–#10016. The code and its own checks are complete; these device and login
steps do not hold the issue open. No owner task, pairing, keychain item, service,
or persistent chat was created. The foreground timing window closes after
writing its measurements. The ordinary offline chat fixture remains available.
