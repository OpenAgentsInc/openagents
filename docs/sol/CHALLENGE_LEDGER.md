# Sol challenge and falsifier ledger

- Date: 2026-07-09
- Class: contract
- Dispatch: no. Challenges inform current authority but do not select work
- Owner: Sol roadmap
- Updated: 2026-07-12 (July 9 falsifier extraction)
- Status: active roadmap-governance ledger
- Canonical roadmap: [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md)

Fable challenges strategy from outside the dispatch queue. Sol owns sequencing
and records the disposition of a material challenge so disagreement becomes a
future test rather than disappearing.

Each entry records: challenge, Sol disposition, owning issue, falsifier or
tripwire, and review point. A rejected or deferred challenge must be as easy to
revisit as an accepted one.

| Challenge | Disposition | Owner | Falsifier/tripwire | Review |
| --- | --- | --- | --- | --- |
| Desktop and mobile can fork identity, run state, or command outcomes. | Accepted as the central P0 risk. Khala Sync is canonical continuity. Local stores are caches/queues only. | #8566, #8574, #8597, #8638 | Matching refs/versions/outcomes cannot be reconstructed after restart/handoff, or either client presents local/optimistic state as authority. | Every R1–R4 landing and R7 dogfood. |
| A lost command acknowledgement can cause false success or duplicate execution. | Accepted. Timeout is unknown-pending-reconcile and replay requires idempotency plus durable evidence. | #8574, #8597, #8638 | A timeout renders accepted/effective, a retry duplicates work, or clients disagree after reconciliation. | R3/R4 fault suite and every control-contract change. |
| OpenCode parity breadth can outrun the reliable core. | Accepted. D0–D6 remain required, but R0–R4 contracts and truthful states precede breadth. | #8574 | Placeholder/editor/terminal breadth grows while Sync, Fleet authority, recovery, or clean-state tests remain red. | Every #8574 claim/release. |
| Mobile can become a cramped, unsafe Desktop clone. | Accepted as a design/security risk, not a reason to omit coding. Mobile owns a compact remote workbench plus fleet control. Files/diff/terminal/preview/writeback are typed remote-workroom capabilities, never raw local device authority. | #8597, #8547, #8636 | Desktop columns are squeezed onto a phone. A client gains raw filesystem/process/credential/port authority. Or mobile cannot finish a useful remote coding task without Desktop. | Every R6/workroom slice and R7 dogfood. |
| “Port all Khala Code ideas” can become an unbounded legacy rewrite. | Accepted. The capability ledger requires an explicit port/replace/pause/reject disposition. Effect Native receives behavior/contracts/test vectors, not the deprecated component tree or local authority. | #8597, #8566 | The new app imports the legacy package/UI architecture, silently drops a useful behavior, or treats old pixels/app-local state as the contract. | Every migration wave. |
| Remote containers can be described as secure before real isolation proof. | Accepted. Development container/control-plane rungs may unblock UI fixtures but cannot satisfy R7. Brokered grants, workspace/account isolation, network/port policy, safe writeback, stop/reclaim, and receipts require real proof. | #8547, #8636 | A mock/fake VM is called production proof, grants are replayable, private credentials reach the client, force writeback occurs, or expired work survives reclaim. | M3–M7 and every isolation change. |
| Realtime media/avatar work can consume the reliability program. | Resolved by owner pause. Retain compatibility and incident repair only. No experiments or presentation queue. | #8610, #8646, #8650 | A new A/V/persona/polish slice starts without an explicit owner reactivation or exact R0–R7 blocker. | Every roadmap reconciliation. |
| Blueprint facts need correction, deletion, and provenance export. | Deferred until R7, with an automatic privacy/data-integrity tripwire. | #8642 | First real user requests correction/deletion, or a privacy incident shows an incorrect/over-broad projection. | Immediately on tripwire. Otherwise after R7. |
| Named colleagues/roles can regrow persona-first scope. | Paused. Any future automation consumes the same typed action/authority contracts. No role expansion during R0–R7. | #8643 | Owner explicitly reactivates after R7 with evidence of a distinct authority/scope/responsibility boundary. | Post-R7 only. |
| Fixture-proven Blueprint or fleet work may be described as done before deployment/live acceptance. | Accepted. Six-rung status vocabulary is mandatory. | All roadmap issues | Any report compresses code-landed, fixture-proven, deployed, live-proven, owner-accepted, and closed. | Every closeout/reconciliation. |
| Effect Native can become an elegant framework bottleneck instead of product leverage after Sarah removal. | Accepted. Shared semantics/intents must be pulled by real Desktop/mobile consumers. Renderer fidelity stays behind typed hosts. | #8566, #8574, #8597 | The second/third renderer costs more than a bounded platform implementation. Consumer teams add local primitives/state. Vendored versions drift. Migration changes lines without deleting duplicate authority. Or framework work lacks a two-renderer consumer. | Every shared-component migration. Monthly while Effect Native is an active gate. |
| Unified orchestration can hide custody, capacity, account, cost, or fallback rails. | Accepted. One claim system does not mean one undifferentiated authority/economic pool. | #8547, #8636 | `auto` changes rail silently. Account/target refs disappear. Owner subscription is called marketplace supply. Org-cloud and local failures collapse. Or fallback crosses owner/data/isolation boundaries. | Every target/broker/fallback change and R3/R7 receipt. |
| Exact receipts can still be unusable trust projections. | Accepted. Preserve exact backend evidence while making the bounded projection answer the user's decision. | #8566, #8574, #8597 | A non-developer cannot determine what happened, why it is credible, what it cost, and what to do next in under one minute without raw refs, SQL, or logs. | Every D5/R3 receipt surface and R7 dogfood. |
| High issue/commit velocity can create integration and documentation debt instead of complete loops. | Accepted. Measure completed user loops, deleted alternate paths, live receipts, and authority freshness—not volume alone. | Sol roadmap. #8566 | Closed issues rise while owner gates accumulate. No alternate path is deleted. Roadmap/index/issue prose contradicts live state. Or no weekly result reduces the systems a user must mentally join. | Weekly during P0 and every documentation-cleanup closeout. |
| A temporary named compatibility adapter can become permanent product authority. | Accepted. `/api/sarah/fleet-runs` is a temporary typed adapter only. Neutral clients/contracts are the destination and every bridge needs an owner, expiry, and deletion/rename gate. | #8566 | New client state depends on the Sarah name. No neutral route/contract owner exists. The adapter survives the next release gate without a dated decision. Or it is used to revive Sarah UI/sequencing. | Every R7/release reconciliation until deleted or explicitly renamed. |

New material disagreements append rows. `MASTER_ROADMAP.md` remains concise.
this ledger carries the falsifier history.
