# Decision record — greenfield clients and Sarah removal

- Class: decision
- Decision dates: 2026-07-09 through 2026-07-10
- Consolidated: 2026-07-12
- Status: current binding decision record
- Authority: [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md), owner decisions
  2–7 and 13
- Supersedes as current authority the
  [archived July 9 greenfield decision](https://github.com/OpenAgentsInc/backroom/blob/dec8ae52/archive/openagents-sol-docs-2026-07-12/july9/2026-07-09-greenfield-mobile-desktop-decision.md)

## Decision

OpenAgents mobile and Desktop are greenfield successor products:

| Product | Destination | Application grammar | Host |
| --- | --- | --- | --- |
| OpenAgents mobile | `apps/openagents-mobile` | Effect Native | React Native/Expo |
| OpenAgents Desktop | `apps/openagents-desktop` | Effect Native | Electron |

Both products are named `OpenAgents`. Mobile retains the owner-designated
`com.openagents.app` identifier and exact designated icon bytes. The signed
Desktop renderer is tokenless behind one host-owned Runtime Gateway; React
Native/Expo and Electron are least-authority hosts, not alternate application
models.

The deprecated mobile, iOS, and Electrobun clients are frozen extraction and
migration sources. Their useful behavior, native modules, tests, fixtures,
icons, and service contracts may move into the greenfield destinations only
through current typed boundaries. Their component trees, app-local state
models, release identities, and authority shortcuts are not conversion
targets. A legacy product is deleted or archived only after its successor and
migration/rollback proof satisfy the current roadmap gate.

Sarah is not home in either successor. The Sarah web/mobile product surface,
package, named front door, persona navigation, avatar, video, and presentation
program are removed/closed not-planned. `/sarah/*` remains an explicit 404
tombstone and retired behavior contracts remain evidence. The legacy
server-side `/api/sarah/fleet-runs` intake may remain only as a temporary typed
authority adapter with an explicit rename/deletion gate; it does not authorize
Sarah UI, state, or sequencing. Persona-neutral conversational voice is a
normal typed session modality, not Sarah revival.

## Non-negotiable boundaries

- Effect Native owns shared application components, state semantics, and
  registered intents; platform code owns only host/renderer capabilities.
- Khala Sync and owning services retain durable identity, state, policy,
  execution, money, credential, and receipt authority. No successor imports a
  legacy local authority universe.
- Mobile may complete useful remote coding through brokered workroom
  capabilities but never receives raw device/host filesystem, shell,
  credential, process, port, or vendor-control authority.
- Retirement claims distinguish code-landed, fixture-proven,
  deployed/distributed, live-proven, owner-accepted, and closed.
- Physical iOS remains a required mobile acceptance host; the owner decision
  accepts Android-emulator evidence and nothing gates on physical Android.

## Evidence and history

The archived July 9 document preserves the original greenfield reasoning and
the later-invalidated “Sarah is home” premise. It is historical evidence, not
current authority. Its provenance and removal gate are in the
[`July 9 archive manifest`](../2026-07-12-july9-doctrine-extraction-and-backroom-manifest.md).
