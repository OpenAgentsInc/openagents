# Gateway completion and tool-prefix recovery

Source checkpoint for #13620, following
`96896f67617c72566c618cbd43487ded296d81f9`. The gateway now delivers a successful
terminal event only after a successful complete HTTP response. The shared tool
step can consume the adapter's actual `complete` observation instead of trusting
the model's provisional finish.

The checked Coax completion owner retains terminal metadata across parser arena
resets. It admits at most 1,024 combined provider/reason bytes, rejects duplicate
or late semantic events, and publishes once after clean transport completion.
Failure and an absent finish remain failures after repeated reconciliation.
The native adapter rejects short HTTP bodies, oversized lines and an incomplete
final SSE event. Whole JSON is parsed only after successful curl completion.
The existing adapter bounds still apply; successful prefixes cannot escape a
failed whole response or an exhausted event buffer.

Tool fragments now pass through the existing plan. Once a tool fragment has
been handed to its caller, a refusal ends that attempt instead of retrying it
or appending another provider's fragments to the same candidate. Cleanup
preserves a previously observed successful completion.

The existing public facade and entry points remain. Native process/HTTP effects
move from the codec facade into `coax/lib/gateway/client.coax`; native plan
polling moves into `plan-client.coax`. Pure completion and attempt decisions
remain directly checked Coax. The new policy is `#authority`; the native
adapters are qualified through actual curl and local socket effects.

## Observed qualification

Heavy checks run on the owned Linux pool. Counts overlap through imports.

| Check | Result |
| --- | --- |
| Completion policy gate | 119 checks; 70 changed-procedure mutants, 69 killed and one reviewed equivalent, zero unresolved survivors |
| Gateway facade gate | 169 checks; the same 70/69/one completion mutants, zero unresolved survivors |
| Plan facade gate | 236 checks; 90 mutants, 89 killed and the same one equivalent, zero unresolved survivors |
| Extended native product fixture | Original evaluator plus debug/release, ordinary/ASan/UBSan all pass; 11 seconds on `oa-build-4` |
| Existing Coax socket/retry qualifier | 18 checks pass through real curl and local TCP sockets; six seconds on `oa-build-4` |
| Public CLI build | Passed in 78 seconds on `oa-build-4` |
| Tools, wire and layer checks | Passed; 395 registry rows and unchanged allowlists |

The sole equivalent widens the individual terminal text bound by one byte;
the independent combined text/reason extent check still rejects that byte.
The first policy run found six survivors. Added assertions retain failure/end
state and exact refusal status; only that independently redundant bound remains.

The socket qualifier holds back the final declared response bytes after the
model's finish. It checks that no terminal success is delivered while those
bytes remain outstanding. Separate cases send a valid model finish in a short
HTTP body, a valid whole JSON tool reply in a short body, a late semantic event,+a 70,002-byte comment line and an unterminated final SSE event. Each refuses
without a successful completion. The whole-response failure exposes no tool
event; streamed failures retain the original native call ID as inert progress.
The tool-prefix case enables both retries and a fallback attempt and observes
exactly one served request and a cut outcome.

The earlier standalone smoke driver could not compile its old duplicate `main`
closure and an expanded driver reached its type-checker storage limit. The
final checks extend the existing Coax socket qualifier; no compiler limit or
eligibility rule was weakened. Initial facade mutation runs correctly reported
unreached native process/clock procedures. Separating those effects from pure
policy keeps the policy gate meaningful and gives the effects their actual
native qualification. Mutation counts above qualify policy, not host effects.

The prior 576-second durable API and Responses observations still qualify
their unchanged JSON ownership and contracts; those suites are not repeated
for this native HTTP adapter change. A later main commit updates only the
owner's native chat check record; rebase preserves it without changing these
library procedure bodies.

This closes the transport-completion gap recorded in the previous checkpoint.
Tool execution, strict argument/schema admission, rich transcript adoption,+provider accounts, durable capture/Stop recovery and real staging patch/test
qualification remain #13620 work. The issue stays open. Production and
TestFlight remain deferred under the owner's full-roadmap release condition.
