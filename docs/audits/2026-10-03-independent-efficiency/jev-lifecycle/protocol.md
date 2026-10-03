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

## Metadata span selection follow-up

This separate development experiment selects declaration pointers before loading
source bodies into model state. The earlier preparation records and source
assembler remain unchanged. The exposed alpha, beta, and gamma tasks remain
development evidence; no native outcome may tune this policy after its freeze.

The initial catalog admits at most 64 implementation functions, 16 test functions,
and eight explicitly named contract documents from the pinned public task scope.
It uses qualified names, repository paths, declaration kinds, AST ranges, and
bounded signatures; function bodies do not affect admission. Within the 64
implementation slots, up to 32 are reserved for public operation-shaped functions.
A function qualifies when its signature is externally public and it takes a
non-receiver argument, is async, takes mutable or by-value `self`, or returns a
`Result`. Conventional `new`, `default`, `with_capacity`, `from_*`, and `with_*`
constructors/builders do not enter that reserved tier. This is a declared
syntactic heuristic, not proof of an API's meaning.

The remaining implementation slots use the original lexical name/path order over
all unselected functions, preserving access to private helpers. Tests and documents
use lexical ordering. Each file receives one turn before its remaining functions
inside each selection group. At most 48 scoped Rust files are read for signatures;
immutable blob size and digest checks still apply. Only the first 512 UTF-8 bytes
of each signature enter model state, with truncation explicitly labeled. Git must
read each bounded blob to recover those signatures; this is not zero source I/O.
All cap and parse omissions are counted. Test roles come from test paths and
namespaces and do not establish that a function is an executable test. Types and
constants are outside this function-pointer catalog.

Each exact task sentence receives an independent Choice over the same catalog
plus `none`. An optional Score per pointer supplies a second ordering signal.
Sentence splitting preserves the public task; it does not infer a complete
requirement decomposition. Questions select useful next reads, not satisfied
requirements. Metadata cannot establish the behavior of an unseen body.

Both deterministic and semantic arms use the same catalog and 16,384-byte rendered
budget. The deterministic arm uses lexical clause matches. Code orders pointers
by clause-selection count, then supplied Scores or lexical rank, with stable ties.
It materializes at most 24 source pointers after selection. Exact explicit
contracts receive an identical rendered allowance of at most 3,072 bytes total,
divided equally by document count and placed first. Excerpts are labeled partial;
complete applicable instructions are supplied separately in both native arms.

Code verifies immutable Git blob identities, file size/hash bindings, and line
ranges before rendering. A function of at most 6,000 bytes is retained whole when
it fits. Larger functions receive a labeled, complete-line leading slice of at
most 2,400 bytes and a pointer to the full span. A unit that does not fit is
recorded as omitted. Source text, provenance headers, and fences all count toward
the rendered limit. Catalog state is capped at 64 KiB and refused rather than
silently shortened. Record catalog construction, materialization, call cost, and
call latency separately and include all of them in later native endpoints.

The source-pointer design follows TypeSafe's
[pre-parsed value extraction](https://docs.typesafe.ai/cookbooks/pre_parsed_value_extraction_cookbook):
code enumerates exact candidates and copies the selected source. Its
[hierarchical classification example](https://docs.typesafe.ai/cookbooks/hierarchical_classification)
shows why early candidate pruning can lose a valid destination. This experiment
uses a bounded flat declaration catalog; it does not implement the cookbook's
beam search. The [Choice contract](https://docs.typesafe.ai/primitives/choice)
allows up to 255 options, so this catalog plus `none` fits within that limit.

Before model calls, the exposed-task checks informed two admission revisions.
The first lexical metadata catalog still omitted two low-overlap gamma entry
points. Prioritizing every public operation recovered those entries but displaced
internal helpers. The final policy reserves only 32 implementation slots for the
public-operation tier and fills the remainder lexically. This is development
iteration informed by known source-coverage gaps, not unseen-task validation.

The final catalogs admit all previously recorded named units: alpha four of four,
beta three of three, and gamma four of four. This does not mean the 16 KiB packs
include them: the deterministic pack contains complete `Host::request_enrollment`
for alpha and `Log::append` for beta, and none of gamma's four named boundaries.
The remaining source-selection question is therefore still open. The selector reads only public task and pinned source inputs. Developers had
already inspected these tasks and their known coverage gaps; this remains
development tuning. No native outcome informed the policy.

Single local catalog/materialization observations were 0.675/0.213 seconds for
alpha, 0.344/0.162 seconds for beta, and 0.302/0.185 seconds for gamma. The largest
request, including the optional per-pointer Scores, was 107,144 bytes. These are
pre-call feasibility observations, not repeated latency estimates. Rendered
source uses complete enclosing lines and adjacent single-line attributes or doc
comments; it does not claim to reproduce only the AST's exact byte interval.
