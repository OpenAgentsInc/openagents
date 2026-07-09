# Sol challenge and falsifier ledger

- Date: 2026-07-09
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
| Realtime media capacity can make Sarah's front door rationed or misleading. | Accepted as a parallel availability/economics contract; text remains the floor and media is leased enhancement. | #8610; FC-3 proves fleet usability under media failure. | Frozen/stale media displays LIVE, text is denied for lack of a render slot, or marginal media cost exceeds its measured outcome benefit. | First Sarah fleet canary and every render-capacity change. |
| Avatar experimentation can outrun relationship utility. | Accepted with paired crossover trials and a stop/remove-candidate rule; does not regain serial P0. | #8610 | An experiment cannot name the production decision/threshold it changes, or experiment count grows without rejected candidates. | Alongside first canary cohort, then monthly while active. |
| Blueprint facts need correction, deletion, and provenance export. | Deferred until local fleet cutover, with an automatic privacy tripwire. | P2 BM-CORRECT issue | First real user asks Sarah to correct/delete remembered data, or a privacy incident shows an incorrect projection. | Immediately on tripwire; otherwise after #8640 Phase A. |
| Sarah may need named colleagues instead of unlimited mode switching. | Deferred to evidence; typed relationship/operator posture ships in FC first. | P2 SARAH-ROLES issue | At least two authority/scope/responsibility/audience/metric dimensions diverge and repeated mode-switch tests show confusion or accountability loss. | After Phase A canary evidence. |
| Fixture-proven Blueprint or fleet work may be described as done before deployment/live acceptance. | Accepted; six-rung status vocabulary is mandatory. | All roadmap issues | Any report compresses code-landed, fixture-proven, deployed, live-proven, owner-accepted, and closed. | Every closeout/reconciliation. |

New material disagreements append rows. `MASTER_ROADMAP.md` remains concise;
this ledger carries the falsifier history.
