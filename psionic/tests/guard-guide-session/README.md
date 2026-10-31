# Directions session checks

`verse/everglade/guard-guide-session.coax` owns the explicit Directions
interaction. Its displayed action always comes from the fixed local goal-to-tip
rule. Elephant responses are shadow observations and never select the displayed
line. The data-only page contract is `guard-guide-session-types.coax`; the policy
is linked separately from the large game page.

A new, monotonic intent serial consumes one opportunity: the local line appears
for four seconds, a six-second cooldown prevents another line, and the first
simulation boundary may start one shadow request if eligible. A refusal never queues
a later retry. Shadow requires the literal `guard-ai=directions-shadow` query,
a confirmed authenticated session, known current gameplay and an explicit intent;
its eight-second lease, ten-second cadence and six-attempt visit budget are local
limits in addition to the real service's authentication and quotas.

The current epoch, actor, generation, world revision, goal revision, actual goal
and new-intent serial fence every shadow. Nonboundary lifecycle updates may revoke
but cannot start/admit remote work. Owned callbacks stage only; admission waits
for current post-gameplay facts. The session normalizes a visible hint only when
its owner matches this very same local intent. That ownership survives visual
expiry so the tip's own cooldown cannot invalidate its pending shadow. An
external tip, changed intent, unsafe context or obsolete callback gets no such
exemption. The bridge must cancel a host request as soon as `ggs_pending` becomes
false and retain the sequence beside its HTTP slot.

`ggs_effect` returns the fixed endpoint, exact existing `gg_request` bytes,
cookie-POST flags, sequence/intent and remaining lease. It borrows the session's
bounded body buffer: the host must copy it before the next state transition.
The host reports dispatch once through `ggs_sent`; an unstarted/duplicate/unknown
callback cannot own a later request. Only the Coax session decides whether an
answer remains admissible.

The probe namespace starts at 200. Counters describe actual attempts; ordinary
local-only/guest play does not manufacture remote failures. “Fallback” counts
started shadows closed without admission. An admitted quiet shadow is an
abstention and still leaves the authored local line alone. These observations
establish no benefit over the perfect local reference rule.

| Probe | Meaning |
| --- | --- |
| 200–206 | Version, mode, local lines, shadow starts, replies, admissions, fallback |
| 207–214 | Last raw enum, last admitted enum, terminal reason, terminal count, distinct late callbacks, phase, active sequence, disabled |
| 215–218 | Stored local enum, local intent, highest observed valid intent, cooldown deadline |
| 220–228 | Terminal counts for reasons 1–9 |
| 293–296 | Last request-to-reply, reply-to-admit, request-to-admit milliseconds, clipping flags |
| 400–410 / 420–430 / 440–450 | Corresponding bounded latency histograms |

Reasons are the existing `GM_*` values: admitted, authentication, quota, no
transport, timeout, malformed response, revoked permission, stale semantics and
HTTP failure. A frame prioritizes timeout, then revoked permission, then changed
semantics. An owned HTTP 401/429 is classified as authentication/quota and disables
later shadow attempts even at/after expiry, as in the existing ambient policy.
Other expired callbacks are timeouts; none can be admitted. Unknown probe fields return −1; 240–244 remain
available to the page's separate button diagnostics. The stored local enum is
not a visibility signal: `ggs_view(...).visible` and its zero-when-hidden
`local_action` govern rendering.

Run only this changed slice:

```shell
coax/bin/coax test verse/everglade/guard-guide-session.coax
coax/bin/coax mutate verse/everglade/guard-guide-session.coax --root-only --jobs 4
psionic/tests/guard-guide-session/check-native.sh /tmp/guard-guide-session-native
```

Initial qualification on 2026-10-30 (2026-10-31 UTC): 81 baseline checks, including
27 new session checks; 595/595 entry-source mutants killed, zero survivors. The
independent C oracle passed 105 cases in debug, release and ASan+UBSan. It tests
all five goals against all five raw choices at deadline boundaries, preserves the
local display across quiet/wrong/admitted shadows, covers own/external hints and
context replacement, and checks six-attempt and scalar/span ABI boundaries.
The final extra invalid-future-serial check changed checks only after the native
run; executable policy was unchanged. Existing GG, Gd and ambient authority roots
were reused; their mutation suites and whole-model CPU gates were not repeated.
Actual page/Wasm/browser qualification is a separate integration record.
