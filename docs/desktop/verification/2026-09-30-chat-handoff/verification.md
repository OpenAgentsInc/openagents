# Desktop conversation handoff verification

Issue: [#10015](https://github.com/OpenAgentsInc/openagents/issues/10015).

The phone and desktop share conversation prompt construction, project selection,
and portable task input and receipt validation. The resident host exposes its
own computer as ready when it has its established key source and configured
projects. Its router context advertises that availability. A completed computer
judgment or explicit Coder offer produces **Run Coder** in the native chat.
The same-user broker introduced by #10014 admits the task through the normal
NIP-HOST owner and task-owner checks. It never changes the auto-start policy.

An encrypted handoff plan binds the conversation to the selected project and
exact prompt before dispatch. Its stable task request survives a lost response
and a host restart. The conversation stores the accepted host, task, full bounded
project label, and selection time. Computer judgments survive restart too.
Failed handoffs retry dispatch rather than asking the hosted chat for another
reply. A bound task replaces the dispatch chip with an accepted-task notice;
#10016 adds its live task view and controls.

Project groups use one pass over the catalog. At most 64 project headings appear;
remaining chats retain their project names under **More projects**. A fixture
with 512 distinct projects keeps every chat selectable and fits the semantic
node budget. Chat outcomes use a boxed worker event to keep unrelated events
small as the snapshot contract grows.

## Checks

Rust 1.97.1 on an M5 Max Mac:

- `coder --test desktop_local_coder`: two acceptance checks passed. The real
  desktop socket client reaches a scratch resident host and durable task inbox.
  The handoff keeps the phone's exact prompt and project, invokes the host's
  enabled auto-start policy through an injected launcher, and records a start.
  Restoring an unbound chat record and index after dispatch still returns the
  original task after restart, with one launch. Archived chats and replies with
  no current offer cannot dispatch. The existing admission, journal failure,
  history, exact retry, and cancellation checks also pass.
- Shared chat core: 31 passed, one opt-in check ignored. Shared application:
  77 passed. Native adapter: 47 passed, including the regression that each
  trailing space advances the caret and remains selectable.
- `coder-access`: 37 passed. `coder-computers` library: 66 passed. All four
  scanned-code pairing checks pass after correcting their stale fixture to
  request the existing `Rights::pairing()` contract. Production rights and
  admission are unchanged.
- Desktop library: 81 passed. Binary: 32 passed, four opt-in checks ignored.
  Packaging and pairing checks: three passed. Existing host control: five
  passed. The Grid backdrop and retained painting remain covered.
- Phone library: 142 passed, 19 opt-in checks ignored. Strict phone Clippy
  passed. Targeted formatting and strict all-target Clippy passed for the
  affected workspace crates; strict Clippy for the Coder integration passed.
- Native captures show the offer at default and minimum sizes and the accepted
  task at minimum size: [offer](offer.png), [minimum offer](offer-minimum.png),
  [accepted task](dispatched.png). The current offline Mac fixture was reopened
  through native app controls. Typing `hello` followed by three separate spaces
  advanced the caret on each press without another letter. The unsent probe
  was cleared, and the current fixture remains open.

The optimized foreground matrix uses 3,300 rows, 500 chats, a Grid backdrop,
1,200 × 840 points at 2×, and 115 samples per active phase. The retained
[native measurements](native.json) record p50/p99 CPU work plus submission:
scroll 2.613/4.299 ms, streaming 6.522/7.284 ms, and sidebar 5.085/5.606 ms.
Idle CPU was 3.12% of one core; peak RSS was 231.6 MiB. These measure CPU work
and submission, not GPU execution completion.

All task acceptance state, keys, relay, policy, and traces use temporary roots.
The launcher validates the real execution grant and records the policy's
launch without running an inference engine or accessing a login. The installed
OS key-source and actual engine run remain the owner-only check in
`NEEDS_OWNER.md`, under #10014 and #10015. No owner task, pairing, keychain item,
service, or persistent chat was created. The existing debug Coder link emits
its large unwind-table warning; the optimized desktop build passes.
