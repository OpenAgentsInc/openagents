# Coder for Mac: local Parakeet dictation

This record covers the Mac Zeron composer's hold-to-talk adoption of Psionic's
original-weight Parakeet v3. The public recording and injected capture effects
exercise draft insertion without a send. No owner microphone, permission
dialog, visible window or private audio is used by the automated checks.

| Boundary | Focused evidence |
| --- | --- |
| Consent, focus, timing, silence and tickets | [Controller](controller/README.md): 14 policy groups, 250/250 root mutants killed; 20 composition groups; native O0/O2 and fatal ASan/UBSan. |
| Literal insertion and undo | [Insertion](insertion/record.json): 51 checks including dependencies, 218/218 root mutants killed; native O0/O2 and fatal ASan/UBSan. Stale, cancelled, malformed and oversized text cannot send or alter a draft. |
| Native keys, focus, microphone geometry and status | [UI](ui/record.json): 41/41 root mutants killed; status-text check. |
| Permission, capture, worker ownership and retries | [Native host](native-host/README.md): fourteen injected effect cases, normal and ASan, plus one actual resident Metal recording. |
| Resident audio disposal | [Explicit clear](privacy-clear/README.md): CPU and shared GPU capacities, fatal sanitizer checks, real Metal clear/reuse and Swift success/failure cleanup ordering. |

The native-host receipt precedes the additive clear API; it retains its exact
source snapshots. The privacy receipt qualifies the subsequent cleanup delta
and identifies the installed archive. Earlier failed observations are retained
as failures, with the missing oracles or implementation corrections described.
No full model corpus, whole-repository gate or unchanged mutation run was
repeated. LeakSanitizer is unavailable in these macOS profiles and is not claimed.

The [offscreen application gate](app/README.md) passes with an isolated
2,097,152-work compiler: both real key/mouse callback routes insert the exact
public recording's transcript, Undo restores the prefix, and sent/message
counters stay zero. The PNG was visually inspected; the microphone and draft
status render correctly. No real microphone was opened.
The actual screen callbacks pass **20/20 focused dictation checks**. The full
settings/desk build remains blocked at the default 1,048,576-work limit in
existing imported checks. The repository default stays unchanged pending the
owner's budget decision; no default-profile build or hardware success is claimed.
The compiler-readiness directory preserves the existing desktop borrow-analysis
failure and the bounded compiler correction needed to build the current screen.

The shipping path is default-off. Enabling it opens the microphone only while
the app and composer window are active. Recognition is local; the final text
enters the original unchanged composer as one undoable edit. Escape, editing,
IME, navigation, consent/focus loss and stale tickets invalidate delivery.
There is no dictation call to the send path.

The automated public clip is not a hardware/TCC acceptance test. The owner's
remaining microphone, focus, menu and Undo check is in
[`NEEDS_OWNER.md`](../../../NEEDS_OWNER.md). Device selection, streaming,
repository vocabulary, representative multilingual dictation and measured
release latency remain outside this change. The 60-second path uses complete
bounded offline slices, with no claim of cross-boundary word equivalence.

See [the user guide](../../voice/coder-dictation.md) for controls and model setup.
