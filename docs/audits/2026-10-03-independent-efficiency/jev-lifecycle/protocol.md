# Jev across the coding lifecycle: development pilot

Recorded October 3, 2026, before the component calls in this directory.
Tracking: [#10356](https://github.com/OpenAgentsInc/openagents/issues/10356).

The owner requested deeper Jev integration and specified Vercel AI Gateway.
This pilot explores preparation, evidence gathering, and review using already
exposed development tasks. It is separate from the unsealed
[48-session ranking study](../system-one-delegation/protocol.md). It neither
changes that study's frozen preparer nor counts component calls as coding runs.

## Questions and limits

1. Does deterministic package and file admission recover implementation evidence
   that the earlier lexical pool omitted? Compare candidate coverage before any
   semantic ranking. Complete declarations and explicitly incomplete source
   slices have different labels; a slice never claims to be a complete AST unit.
2. Given the same candidate pool and 16 KiB rendered budget, does Jev's relevance
   ranking deliver useful implementation and contract evidence? Preserve all
   original requirements. Map each exact task clause to a candidate or no match;
   keep the mapping as a reading suggestion, not an executable plan.
3. Can Jev choose a useful additional source read from an enumerated catalog?
   Include a no-read option. Code checks the selected identity and retrieves only
   the pinned public source. No generated shell command runs.
4. Can Jev flag omissions in retained patches that executable checks have already
   evaluated? Give it the public task, patch, and available public context. Keep
   candidate labels, independent checker text, and later explanations out of
   its input. Report sensitivity and false alarms, including an always-review
   baseline. No semantic judgment accepts or rejects a coding result.
5. Does batching independent questions reduce measured API overhead? Compare
   exactly the same four questions and state in one request and four requests,
   in both orders. Retain disagreements as well as cost and latency.

The four preparation tasks are `reserved-a`, `alternative-alpha`,
`alternative-beta`, and `alternative-gamma` from the
[public task manifest](../system-one-delegation/replacement-public-task-manifest.json).
They are development cases already examined by the audit. Review cases come
from retained patches with executable diagnostic labels. Selection and state
construction are recorded before inference. There is no unseen quality claim.

The first round permits at most 40 HTTP requests, each with a 30-second socket
timeout and no automatic retry. Requests use only `typesafe-ai/jev` at Vercel's
TypeSafe endpoint. Do not enable evaluation fallbacks. Keep the exact request,
response, provider routing metadata, and elapsed time for every attempt. Missing
usage is unknown, not zero. Count gateway `cost` once; other cost fields overlap.
The gateway alias has no verified weight/version pin. Report its actual provider
attempts even when the client makes only one HTTP request.

Review's primary positive prediction is the typed `next_action: revise` choice.
`inspect` is an abstention and `verify` requests executable checks; neither
certifies correctness. Also report the separate `material_defect` probability
and its descriptive classification at 0.5. This development threshold grants
no execution authority and is not a calibrated production gate. Keep both
judgments when they disagree. Report counts against the independently recorded
labels, including abstentions, rather than dropping hard cases.

## Interpretation

All phases are independently testable and runnable without starting a coding
agent. A coding trial must later include their latency, added executor context,
probe cost, and any revision turn. Cheap judgments alone do not establish cheaper
accepted patches. A static patch judgment cannot demonstrate that an executor
would repair a defect correctly.

The intended next native ablation holds one executor model, tool boundary, source,
checks, and checkpoint opportunities constant: bare native, deterministic lean,
lean plus Jev preparation, lean plus Jev runtime decisions, and both Jev stages.
Use fresh tasks after freezing the resulting policy. The earlier per-step Jev
ablation was slower and less successful, so test bounded checkpoints rather than
assuming that more calls improve results.

The design follows the distinction between finding an item and establishing
that relevant evidence exists in TypeSafe's
[semantic search example](https://docs.typesafe.ai/cookbooks/semantic_find), and
its [independent question batching](https://docs.typesafe.ai/patterns/fan-out).
The [gateway contract](https://vercel.com/docs/ai-gateway/sdks-and-apis/typesafe)
defines this transport and its usage fields.
