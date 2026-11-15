# Chat admission and native task identity

2026-11-15. Source checkpoint for
[#13620](https://github.com/OpenAgentsInc/openagents/issues/13620), following
native SDK offers in `8c429e52879632508a68f86bdbb8162964952e0f`.

Every newly admitted durable account/Project turn now retains a primary native
Input in the same transaction as its hosted inference input, saved history,
operation identity and Project snapshot. Later Project edits cannot replace
those instructions or history. Historical turns without native Input remain
explicitly unavailable rather than being reconstructed from current state.

The account API and built CLI read this input and any retained immutable
TurnTask association. The association composes the existing Binding, actual
SDK offer and authenticated station report with the original router decision,
its exact native dispatch projection and an enrollment digest. Host aliases
come from full enrolled keys; native hostname IDs stay in the original report.
All nested records use their primary codecs. The new protocol is separate
from the published chat/router/execution versions.

Pure Coax admission validates report identity/age, resources, hard needs,
authentication and the exact ready offered model/effort/account. The existing
typed router supplies the computer/engine/profile decision; a typed choice
resolves actual offered model/effort pairs without catalog-order defaults.
Empty optional profile hints are omitted instead of producing invalid empty
decision input blocks. More than 255 executable pairs refuses explicitly.

## Qualification

| Gate | Observation |
| --- | --- |
| Protocol | 88 owned schemas and all 30 frozen compatibility checks passed, including the new `chat-task` consumer closure and source pins |
| Native descriptor/codec oracle | 22/22 checks; exact independent fixture bytes, required/closed fields, nested primary mapping, report digest, bounded output, authenticated full host identities and the existing typed router |
| Native input/lease authority | 205 checks; 251 changed-procedure mutants, 245 killed and six reviewed equivalents, zero unresolved survivors |
| Task association authority | 197 checks; 212 changed-procedure mutants, 205 killed and seven reviewed equivalents, zero unresolved survivors |
| Actual model choice authority | 218 checks; 271 changed-procedure mutants, 269 killed and two reviewed equivalents, zero unresolved survivors |
| HTTP/database/built CLI | 91/91 durable chat cases, 72/72 CLI auth cases and 77/77 existing animation API cases; selected PostgreSQL/ASan/UBSan harness passed in 526 seconds, including native process adapters and 33,286 NIP-98 comparisons with zero disagreements |
| Migration/schema | Applied all 215 migration files and regenerated the checked schema snapshot |
| Tools/wire/layers | Passed: 2,033 tracked tool files, 386 registry rows, one existing wire allowlist line and 30 existing context loads; no increase |

The HTTP fixture reads complete admitted history, changes Project instructions
and compares retained native Input byte for byte through the built signed CLI.
It inserts an explicitly inert association to exercise the primary reader,
owned turn/conversation checks, foreign-account refusal and corrupt-record
refusal. That insertion starts no native task and is not execution evidence.

The generated environment unit keeps shared SHA exports in its existing C
namespace, preventing collision with the NIP-98 and Project/chat units. The
normal API test harness links the complete service and exercises the native
process adapters under ASan/UBSan on the owned Linux build pool.

## Remaining issue scope

No ordinary chat yet produces an association or starts native work. This
checkpoint supplies its admitted bytes and validated identity/read seam.
Server-owned Prepare/upload/Start, background status/Stop reconciliation,
native transcript projection and installed mobile task/permission controls
remain required. Shared gateway tool execution, actual supported SDK catalog
pagination/account selection and a real staging patch/test task also remain
part of #13620. Its end-to-end acceptance boxes stay unchecked.

No provider inference, installed mobile native execution, production rollout
or TestFlight delivery is claimed by these fixtures. TestFlight stays deferred
until the full Claude-port and Arcade work required by the owner is complete.
