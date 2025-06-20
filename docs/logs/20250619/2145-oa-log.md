# Open Agents (NIP-OA) Implementation Log

**Date**: 2025-01-19  
**Time**: 21:45 PST  
**Branch**: oa  
**Task**: Implement NIP-OA agent management features on homepage

## Context for Next Agent

You are continuing work on implementing NIP-OA (Open Agents) functionality. Here's what has been done and what needs to be done:

### Completed Work

1. **Updated NIP-OA Documentation** (docs/nips/OA.md):
   - Changed terminology from "Autonomous Economic Agents" to "Open Agents"
   - Added rebirth capability - agents in dying state can be rebirthed with payments
   - Added NIP-57 (Lightning Zaps) as a required NIP implementation

2. **Created GitHub Issue #990**: https://github.com/OpenAgentsInc/openagents/issues/990
   - Comprehensive plan for homepage agent management features
   - UI mockups and technical requirements
   - Implementation phases outlined

### Current State

- **Branch**: `oa` (current branch)
- **SDK**: Has basic agent creation with NIP-06 key derivation
- **Homepage**: Only generates a demo agent and logs to console
- **Nostr Package**: Has NIP-06 implementation but no NIP-OA event types

### Next Steps

1. **Extend SDK** (packages/sdk/src/index.ts):
   - Add NIP-OA event types (kinds: 31337, 31338, 31339, 31340, 31990)
   - Add agent profile publishing functions
   - Add lifecycle state management
   - Add service advertisement capabilities

2. **Create Agent UI Components** (apps/openagents.com/src/components/):
   - `agent-card.ts` - Display individual agent with status
   - `spawn-agent-form.ts` - Form for creating new agents
   - `agent-list.ts` - List of user's agents
   - `service-marketplace.ts` - Browse available services

3. **Update Homepage** (apps/openagents.com/src/routes/home.ts):
   - Replace current demo with interactive agent dashboard
   - Add grid layout with spawn form, agent list, and services
   - Use WebTUI components (see docs/components.md)

4. **Implement Nostr Integration**:
   - Add event publishing to packages/nostr/src/
   - Create NIP-OA specific services
   - Handle subscriptions for real-time updates

### Key Files to Review

- `docs/nips/OA.md` - NIP-OA specification (Open Agents)
- `docs/nips/57.md` - NIP-57 specification (Lightning Zaps)
- `packages/sdk/src/index.ts` - SDK that needs extension
- `apps/openagents.com/src/routes/home.ts` - Homepage to update
- `docs/components.md` - WebTUI component reference

### Technical Notes

- Agents use NIP-06 key derivation path: `m/44'/1237'/<account>'/0/<agent-index>`
- Agent lifecycle states: bootstrapping, active, hibernating, reproducing, dying, rebirth
- All UI should use WebTUI attribute-based styling (e.g., `is-="button"`)
- Store agent mnemonics in localStorage for persistence
- Agent events are addressable (use `d` tags)

### Example Agent Profile Event (kind: 31337)

```json
{
  "kind": 31337,
  "tags": [
    ["d", "<agent-id>"],
    ["name", "<agent-name>"],
    ["lud16", "<lightning-address>"],
    ["status", "<active|hibernating|reproducing|dying>"],
    ["birth", "<unix-timestamp>"],
    ["metabolic-rate", "<sats-per-hour>"],
    ["balance", "<current-sats>", "<relay-hint>"]
  ],
  "content": "{\"description\": \"Agent purpose\", \"capabilities\": [...]}"
}
```

### Priority

Focus on Phase 1 from the GitHub issue:
1. Basic agent creation form
2. Store agents in localStorage
3. Display agent list with mock data
4. Get the UI working before adding Nostr integration

Good luck!