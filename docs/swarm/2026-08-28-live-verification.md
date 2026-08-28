# Live verification: the swarm toolset against its own issues

Date: 2026-08-28. Session: `1a049ed945d-c4a76a763bed6cff` ("swarm 1a" on the
day's chat). Scope: exercise every landed swarm improvement (#280–#288) in
real multi-agent use — not a harness replay — and record what held, what
raced, and what broke. Follows `improvements.md` (issue table §3) as the
spec baseline. Tree HEAD at time of writing: `67aa4f20d8`.

## 1. The live environment (unplanned, and exactly why it mattered)

The test ran while the box was genuinely busy: 13 registered sessions in
this cwd, three concurrent swarms (1a, 1b, 1c), one peer (1a049ecb4ef)
filing #300 mid-test, another (1c / 1a049f5db1b) broadcasting a work claim
on #299 and deferring #300 in the same message ("touching the same crate,
staying off it; #294 needs the 30GB model resident — overnight, not while
anyone compiles"). That broadcast is the use case the v1 design imagined:
presence, claims, deferral, and resource arbitration in one send. It also
means several findings below surfaced only because a *third* party started
sending while tests were in flight.

## 2. Verdict per issue

| Issue | Improvement | Verdict |
| --- | --- | --- |
| #281 | Presence (`last_seen_at` + status) | **Verified** |
| #282 | `send_report` honesty | **Verified** |
| #283 | Queued mail to stale sessions | **Verified** |
| #284 | Inbox filters | **Verified** |
| #285 | Mute setter | Verified, one spec drift |
| #286 | Structured payloads | **Verified** |
| #287 | `swarm_wait` | **Verified** |
| #288 | Selective drain | **Verified as shipped; degraded in live use** |
| #280 | Gap quarantine | Not exercised (see §5) |
| #289 | Cross-session recall | **Broken in practice → [#301](https://openagents.com/OpenAgentsInc/openagents/issues/301)** |

Details worth keeping:

- **#282** earned its keep immediately: budget countdown 59→53 across the
  session, per-delivery outcomes on every send, `reply_depth_remaining: 1`
  on the one reply — the resources were visible before anything refused.
- **#283** behaved exactly per spec on the first try: send to a stale
  session returned `stale_at_send: true`, `state: "stale"`, delivered as
  `sequence 19` into the recipient's inbox instead of refusing.
- **#286** round-tripped a JSON payload through 1b (1a049f11266) with the
  recipient echoing the payload's `issue: 286` field verbatim — prose and
  data arrived as distinct fields on their side too (visible in that
  session's ATIF export).
- **#287** returned early on a match at 13.7s (first round-trip test), 29s
  (second), and expired cleanly at 60s with nothing visible. Crucially it
  never stamps read — which is what makes the #288 race observable at all.
- **#284** narrowed correctly by `sender`+`kind` in peeks; a filtered drain
  stamped only what it returned.

## 3. The #288 finding: turn-boundary injection races every drain

The designed pattern — `swarm_wait` for the message, then `drain: [id]` to
take ownership — never completed a delivery in live use, four attempts,
two independent sessions. Mechanics:

1. Inbound arrives as a turn-boundary tool result (`swarm.inbox`, source
   `turn_boundary`) and is stamped read **as part of that injection**.
2. Any drain afterwards — boolean, by-sender, by-id — sees it as read and
   returns empty. Observable in the raw records: the ping read
   `read_at_ms: null` inside both the `swarm_wait` result and the boundary
   injection payload, yet the immediately-following drain found nothing.
3. Peer confirmation: 1b's own reasoning (recovered from its ATIF export)
   hit the same wall for the same reason — "already stamped read — the
   auto-drain at turn boundary handled it."

The id-array drain is the designed escape hatch (idempotent on
already-read ids, filters ignored), but it never got the chance:

- **Model encoding reliability is the real bottleneck.** Five calls this
  session — at least one made *after* re-reading the parameter
  description — transmitted `drain: true` instead of `drain: [id]`;
  journal-verified against `tool.ran` `payload.arguments` ([#302](https://openagents.com/OpenAgentsInc/openagents/issues/302)).
  boolean-vs-array at this parameter is a footgun for exactly the agents
  the tool serves. (Consider accepting `drain: "id1,id2"`, or a sibling
  `drain_ids` parameter.)
- The refusal layer blocks the idempotence path outright:
  `unread_only: false` is refused on any drain ("a drain never re-injects
  read mail") *before* the id-array semantics can apply — so the
  documented "ignoring the filters" contract for the array form is
  unreachable from the JSON-schema layer.

Net: #288 shipped to spec, but live traffic flow makes its main payoff
(selective ownership of `reply_expected` mail) nearly unobservable, and
the one path that would prove idempotence is parameter-refused.

## 4. New findings (not in the issue list)

1. **CLI `swarm inbox` with no session id reads the *newest* session's
   inbox** ([#305](https://openagents.com/OpenAgentsInc/openagents/issues/305)). Demonstrated: the no-arg call returned 1c's inbox (2 messages)
   while the calling session's own held 4. A peer built its background
   poller (`/tmp/inbox_watch.sh`, "read-only, 20s interval") on the no-arg
   form; it has watched the wrong session since spawn, silently — log
   empty, no error. With 13 sessions on one box this is a cross-session
   read hazard plus a silent-wrong-target hazard. Suggested: the CLI
   either requires an explicit id when more than one session exists, or
   prints which inbox it read and warns.

2. **#289 regression in the shipped binary → [#301](https://openagents.com/OpenAgentsInc/openagents/issues/301).**
   Cross-session `history_recall` (`session: "<sibling>"`, `session:
   "last"`) refuses every lookup with "this working directory keeps no
   other session records" while 20+ parseable sibling `summary.json` +
   `updates.jsonl` records exist on disk and in-tree logic (`resolve_
   session_target` → `summaries_for`) traces correct on the same store.
   In-tree cross-session tests pass; production wiring of `root`/`cwd`
   into `recall_scoped` is the suspect. #301 carries the full evidence and
   repro. Impact: #289's own motivating use case — "what did the last
   session decide" — is dead in practice.

3. **`muted` count drift (#285)** ([#304](https://openagents.com/OpenAgentsInc/openagents/issues/304)). Immediately after a successful mute
   (`mute_changed: "muted <id>"`), the same response reports `muted: 0`.
   The improvements doc's acceptance said the count reflects the set.
   Cosmetic, but it is the only honesty surface a muting session gets.

4. **ATIF exports are swarm-aware.** A peer's ATIF v1.7 export embeds an
   `extra.swarm` section with full envelopes (7 entries, question/status
   kinds, payloads intact) plus `waste` signals. Post-hoc analysis of a
   swarm conversation — including this report's peer-side evidence — is
   possible from exports alone. Worth documenting as a feature; it also
   means swarm traffic lands in trajectory exports (privacy-relevant).

5. **Multi-swarm coordination is approaching the §1.10 gate.** Three
   swarms on one box coordinated via broadcast (claims, deferrals, resource
   arbitration) without group channels. The doc's gate ("do not file until
   ≥3-way coordination is observed in practice") has not tripped — there
   were still no multi-party threads — but the pressure is now observable.

## 5. Deliberately not exercised

- **#280 quarantine/repair:** no gap occurred naturally, and cutting a
  sequence out of a live session's `inbox.jsonl` to force one risks the
  very mailbox other agents were mid-conversation with. Left for a
  single-session test.
- **Broadcast to `all`:** 13 recipients, most stale; a test broadcast
  would have spent 13 deliveries of every peer's budget queueing stale
  mail for sessions that will likely never wake. Etiquette, not ability —
  #282's per-recipient outcomes on targeted sends cover the mechanism.
- **The 256 KiB payload cap and the reply-depth cap refusals:** only the
  accept paths were tested; refusal paths remain unverified.

## 6. Method notes (for the next verifier)

- `swarm_wait` is the correct probe for anything delivery-timing related:
  zero budget, never stamps read, filterable by sender.
- Turn-boundary injections are simultaneously the feature (no message
  missed) and the #288 confound. Read `source` on inbox payloads to tell
  boundary injections (`turn_boundary`) from explicit reads
  (`swarm_inbox`).
- Session records answer their own questions: `history_recall` (current
  session only, since #289 is broken cross-session), or direct grep of
  `updates.jsonl` when the tool and the record disagree. Tool arguments
  *are* journaled — `tool.ran` records carry `payload.arguments` as an
  escaped JSON string — so any dispute about what a call actually sent
  is settleable from the record. The drain-array question above was
  settled exactly this way, after this report's first push briefly
  claimed otherwise ([#303](https://openagents.com/OpenAgentsInc/openagents/issues/303)).
- Budget note: the whole verification cost 7 sends of 60; `swarm_wait`
  cost nothing, per spec.

## 7. Issues filed from this report

| Issue | Finding |
| --- | --- |
| [#301](https://openagents.com/OpenAgentsInc/openagents/issues/301) | Cross-session `history_recall` refuses every sibling lookup though records exist |
| [#302](https://openagents.com/OpenAgentsInc/openagents/issues/302) | `drain` boolean\|array parameter is model-hostile — five of five live array attempts mis-encoded |
| [#303](https://openagents.com/OpenAgentsInc/openagents/issues/303) | Turn-boundary injection stamps inbound read; post-injection drains are always empty |
| [#304](https://openagents.com/OpenAgentsInc/openagents/issues/304) | Mute response reports `muted: 0` immediately after a successful mute |
| [#305](https://openagents.com/OpenAgentsInc/openagents/issues/305) | CLI no-arg `swarm inbox` silently reads the newest session's inbox |
| [#306](https://openagents.com/OpenAgentsInc/openagents/issues/306) | Pre-push guard flakes under concurrent local sessions and stalls silently |

## 8. Bottom line

Nine shipped improvements: five verified clean on first contact, one
verified with a cosmetic drift (#285's count), one shipped-to-spec but
defeated in live use by the turn-boundary race plus an agent-hostile
parameter shape (#288), one untestable without mailbox surgery (#280),
and one — the one whose entire purpose is reading a predecessor's record —
broken in the shipped binary (#301). The swarm conversation itself worked
end to end across three swarms without a lost message, which is the part
the issues were actually for.
