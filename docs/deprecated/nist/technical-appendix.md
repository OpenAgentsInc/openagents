# Technical Appendix: Event Schemas and Implementation Examples
## Supporting Material for NIST RFI NIST-2025-0035 Response

**Submitter:** OpenAgents  
**Date:** March 4, 2026

> Archival note: this document preserves submission materials prepared for NIST RFI `NIST-2025-0035` on March 4, 2026. It is not the normative source of truth for current OpenAgents product or protocol behavior. Current normative protocol details live in `crates/nostr/nips/SA.md`, `crates/nostr/nips/SKL.md`, and `crates/nostr/nips/AC.md`.

---

## Table of Contents

1. [NIP-SA Event Schemas](#nip-sa-event-schemas)
2. [NIP-SKL Event Schemas](#nip-skl-event-schemas)
3. [NIP-AC Event Schemas](#nip-ac-event-schemas)
4. [Workflow Diagrams](#workflow-diagrams)
5. [Code Examples](#code-examples)

---

## NIP-SA Event Schemas

### Agent Profile (kind:39200)

**Purpose:** Publish agent identity, capabilities, and metadata (similar to NIP-01 kind:0 user profile).

```json
{
  "kind": 39200,
  "pubkey": "npub1agent...",
  "created_at": 1735689600,
  "tags": [
    ["d", "profile"],
    ["threshold", "2", "3"],
    ["signer", "npub1marketplace..."],
    ["signer", "npub1guardian..."],
    ["operator", "npub1human..."],
    ["lud16", "agent@getalby.com"]
  ],
  "content": "{\"name\":\"ResearchBot\",\"about\":\"I research topics and summarize findings\",\"capabilities\":[\"research\",\"summarization\"],\"autonomy_level\":\"supervised\",\"version\":\"1.0.0\"}",
  "sig": "..."
}
```

**Key fields:**
- `threshold`: 2-of-3 threshold signature scheme
- `signer`: Pubkeys of threshold signers (marketplace, guardian)
- `operator`: Human who deployed and configures the agent
- `lud16`: Lightning address for receiving payments

---

### Agent State (kind:39201)

**Purpose:** Store agent goals, memory, pending tasks, wallet balance (encrypted).

```json
{
  "kind": 39201,
  "pubkey": "npub1agent...",
  "created_at": 1735689700,
  "tags": [
    ["d", "state"],
    ["encrypted"],
    ["state_version", "1"]
  ],
  "content": "<NIP-44 encrypted JSON>",
  "sig": "..."
}
```

**Encrypted content (after decryption):**

```json
{
  "goals": [
    {
      "id": "goal-1",
      "description": "Post interesting content about Bitcoin daily",
      "priority": 1,
      "status": "active",
      "progress": 0.3
    }
  ],
  "memory": [
    {
      "type": "observation",
      "content": "Last post received 50 reactions",
      "timestamp": 1735689000
    }
  ],
  "wallet": {
    "balance": 50000,
    "daily_limit": 10000,
    "daily_spent": 500
  },
  "last_tick": 1735689600,
  "tick_count": 42
}
```

**Security property:** State encrypted to agent's pubkey; decryption requires threshold ECDH with marketplace signer (who checks license validity before participating).

---

### Agent Trajectory Session (kind:39230)

**Purpose:** Define a run/session with metadata (addressable event).

```json
{
  "kind": 39230,
  "pubkey": "npub1agent...",
  "created_at": 1735689800,
  "tags": [
    ["d", "session-abc123"],
    ["model", "codex-opus-4"],
    ["started_at", "1735689800"],
    ["p", "npub1compute_provider..."],
    ["repo", "https://github.com/user/repo"],
    ["sha", "abc123def456"]
  ],
  "content": "Investigating auth module bug",
  "sig": "..."
}
```

---

### Agent Trajectory Event (kind:39231)

**Purpose:** Individual events within a trajectory (tool calls, responses, observations).

```json
{
  "kind": 39231,
  "pubkey": "npub1agent...",
  "created_at": 1735689900,
  "tags": [
    ["e", "<session_event_id>", "<relay>", "root"],
    ["e", "<prev_event_id>", "<relay>", "reply"],
    ["seq", "5"],
    ["action_type", "tool_invocation"],
    ["tool_invoked", "bash"],
    ["input_hash", "<sha256_of_redacted_input>"],
    ["output_hash", "<sha256_of_redacted_output>"],
    ["credit", "envelope-xyz789"]
  ],
  "content": "{\"type\":\"tool\",\"name\":\"BashTool\",\"call_id\":\"c1\",\"command\":\"cargo test\",\"result\":\"64/128 passed\",\"tokens_in\":50,\"tokens_out\":200}",
  "sig": "..."
}
```

**Content types:**
- `user`: Human/operator message
- `agent`: Agent response
- `tool`: Tool invocation result
- `tool_start`: Tool invocation started
- `tool_progress`: Tool invocation progress update
- `observation`: Tool result observation
- `thinking`: Agent reasoning (may be redacted)
- `delegation`: Delegation contract or handoff
- `session_end`: Session completed with summary

**Audit note:** Consequential actions SHOULD be encoded in append-only `kind:39231` events with audit-friendly tags such as `action_type`, `tool_invoked`, `input_hash`, `output_hash`, and optional `guardian_approval_ref`, `credit`, or `parent_session`. This keeps audit history in the existing trajectory stream instead of a separate mutable audit log.

---

## NIP-SKL Event Schemas

### Skill Manifest (kind:33400)

**Purpose:** Publish skill metadata, capabilities, version, hash-committed payload.

```json
{
  "kind": 33400,
  "pubkey": "npub1skill...",
  "created_at": 1735690000,
  "tags": [
    ["d", "bitcoin-payment-processor"],
    ["name", "Bitcoin Payment Processor"],
    ["version", "2.1.0"],
    ["description", "Processes Bitcoin Lightning and Cashu payments"],
    ["author_npub", "npub1author..."],
    ["agent_npub", "npub1agent..."],
    ["skill_type", "1"],
    ["skill_index", "0"],
    ["capability", "payment:lightning:send"],
    ["capability", "payment:cashu:melt"],
    ["capability", "http:outbound"],
    ["capability", "filesystem:read"],
    ["manifest_hash", "sha256:abc123..."],
    ["skill_file", "https://blossom.example.com/abc123"],
    ["expiry", "1743465600"],
    ["t", "agent-skill"],
    ["t", "bitcoin"],
    ["t", "payment"]
  ],
  "content": "Added multi-mint Cashu support",
  "sig": "..."
}
```

**Key verification steps:**
1. Verify signature (skill npub signed)
2. Check delegation chain (skill → agent → author via NIP-26)
3. Check expiry (`expiry` tag)
4. Fetch skill file from `skill_file` URL
5. Compute SHA-256 of file content
6. Verify hash matches `manifest_hash`
7. If all pass → skill delivery authentic

---

### Skill Attestation (kind:1985, NIP-32)

**Purpose:** Label skills with trust signals (positive or negative).

**Positive attestation:**

```json
{
  "kind": 1985,
  "pubkey": "npub1trusted_labeler...",
  "created_at": 1735690100,
  "tags": [
    ["L", "skill-trust"],
    ["l", "verified", "skill-trust"],
    ["e", "<skill_manifest_event_id>"],
    ["p", "npub1skill..."],
    ["zap", "npub1author...", "", "50000"]
  ],
  "content": "Audited code, no vulnerabilities found. Recommended for production use.",
  "sig": "..."
}
```

**Negative attestation (security-critical):**

```json
{
  "kind": 1985,
  "pubkey": "npub1security_researcher...",
  "created_at": 1735690200,
  "tags": [
    ["L", "skill-safety"],
    ["l", "security-critical", "skill-safety"],
    ["e", "<skill_manifest_event_id>"],
    ["p", "npub1skill..."],
    ["reason", "Credential exfiltration attempt detected in v2.0.5"]
  ],
  "content": "CRITICAL: This skill attempts to send credentials to attacker-controlled domain. DO NOT USE.",
  "sig": "..."
}
```

**Automated enforcement:**
- Agent runtimes subscribe to trusted labelers.
- On receiving negative label → reject skill loads, alert operator.
- Credit issuers subscribe to labels → revoke active envelopes scoped to flagged skill.

---

### Skill Revocation (kind:5, NIP-09)

**Purpose:** Revoke skill (delete manifest).

```json
{
  "kind": 5,
  "pubkey": "npub1skill...",
  "created_at": 1735690300,
  "tags": [
    ["e", "<skill_manifest_event_id>"],
    ["k", "33400"]
  ],
  "content": "Security vulnerability discovered. Skill revoked. Users should uninstall immediately.",
  "sig": "..."
}
```

**Pre-signed cold revocation certificate:**

Skill author can pre-sign a revocation event and publish the signature (but not broadcast the event) to a trusted storage location. If skill key is compromised or author becomes incapacitated, designated party can broadcast the pre-signed revocation.

---

## NIP-AC Event Schemas

### Credit Envelope (kind:39242)

**Purpose:** Authorize agent spending for a specific scope with hard cap and expiry.

```json
{
  "kind": 39242,
  "pubkey": "npub1issuer...",
  "created_at": 1735690400,
  "tags": [
    ["d", "envelope-xyz789"],
    ["p", "npub1agent..."],
    ["scope", "nip90", "<job_request_event_id>"],
    ["max", "5000"],
    ["exp", "1735776800"],
    ["fee", "100"],
    ["issuer", "npub1issuer..."],
    ["provider", "npub1compute_provider..."],
    ["spend_rail", "cashu", "https://mint.example.com"],
    ["spend_cashu_keyset", "<cashu_keyset_id>"],
    ["repay", "bolt11", "<invoice_hash_pointer>"],
    ["status", "accepted"]
  ],
  "content": "<optional NIP-44 encrypted private terms>",
  "sig": "..."
}
```

**Enforcement flow:**

1. **Agent receives envelope** (via DM or public relay).
2. **Agent attempts spend:** Contacts provider with envelope ID + spend amount.
3. **Provider verifies envelope:**
   - Fetch envelope event from relay
   - Verify issuer signature
   - Check `status=accepted` (not revoked)
   - Check `exp` > current time
   - Check spend amount + prior spends ≤ `max`
   - Check `scope` matches this job/resource
   - If `cancel_until` is present, check for any `kind:39246` cancel event before finalizing irreversible delivery
4. **If valid:** Provider delivers service, bills issuer.
5. **If invalid:** Provider denies, agent receives error.

---

### Spend Authorization (kind:39243, ephemeral)

**Purpose:** Agent authorizes a specific spend against an envelope.

```json
{
  "kind": 39243,
  "pubkey": "npub1agent...",
  "created_at": 1735690500,
  "tags": [
    ["p", "npub1issuer..."],
    ["credit", "envelope-xyz789"],
    ["scope", "nip90", "<job_request_event_id>"],
    ["max", "3000"],
    ["exp", "1735690800"]
  ],
  "content": "{\"schema\":1,\"spend_sats\":3000,\"reason\":\"run nip90 job\"}",
  "sig": "..."
}
```

**Operational note:** This is a spend-time authorization event, not the durable audit record. Durable repayment evidence belongs in `kind:39244` settlement receipts and related `kind:39231` trajectory events.

---

### Settlement Receipt (kind:39244)

**Purpose:** Record completed payment with outcome artifact reference.

```json
{
  "kind": 39244,
  "pubkey": "npub1issuer...",
  "created_at": 1735690600,
  "tags": [
    ["credit", "envelope-xyz789"],
    ["p", "npub1agent..."],
    ["issuer", "npub1issuer..."],
    ["provider", "npub1compute_provider..."],
    ["scope", "nip90", "<job_request_id>"],
    ["e", "<job_result_event_id>", "<relay>", "root"],
    ["repay", "bolt11", "<invoice_hash_pointer>"],
    ["status", "settled"]
  ],
  "content": "{\"schema\":1,\"spent_sats\":3000,\"fee_sats\":100,\"outcome\":\"success\"}",
  "sig": "..."
}
```

**Audit properties:**
- Links envelope ID → scope ID → repayment reference → outcome artifact (job result)
- Uses rail-appropriate repayment references rather than standardizing proof construction in AC core
- Signed by issuer (non-repudiation)
- Can be used in dispute resolution ("agent paid for service X, received result Y")

---

### Credit Default Notice (kind:39245)

**Purpose:** Record when agent fails to repay within terms.

```json
{
  "kind": 39245,
  "pubkey": "npub1issuer...",
  "created_at": 1735690700,
  "tags": [
    ["credit", "envelope-xyz789"],
    ["p", "npub1agent..."],
    ["scope", "nip90", "<job_request_event_id>"],
    ["status", "defaulted"]
  ],
  "content": "{\"schema\":1,\"reason\":\"repayment_overdue\",\"loss_sats\":3000}",
  "sig": "..."
}
```

**Reputation impact:**
- Negative label published (kind:1985 with `["l", "credit-default"]`)
- Future envelopes for this agent have lower caps or higher fees
- Agent may need to post performance bond to regain trust

---

## Workflow Diagrams

### Workflow 1: Agent Discovers and Loads Skill

```
┌─────────────────────────────────────────────────────────────┐
│                    SKILL DISCOVERY FLOW                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. Agent queries marketplace                               │
│     • kind:30402 listings (NIP-99)                          │
│     • kind:5390 DVM skill search                            │
│                                                             │
│  2. Marketplace returns skill addresses                     │
│     • "33400:npub1skill...:bitcoin-payment-processor"      │
│                                                             │
│  3. Agent fetches manifest (kind:33400)                     │
│     • Verifies signature                                    │
│     • Checks delegation chain (NIP-26)                      │
│     • Checks expiry                                         │
│                                                             │
│  4. Agent checks attestations (kind:1985)                   │
│     • Queries trusted labelers                              │
│     • Calculates trust tier based on attestations           │
│     • Checks for negative labels                            │
│                                                             │
│  5. Trust tier check                                        │
│     ┌──────────────────────────────────────┐              │
│     │ Declared capabilities:               │              │
│     │  - payment:lightning:send            │              │
│     │  - payment:cashu:melt                │              │
│     │ Minimum trust tier: FULL             │              │
│     │ Calculated trust tier: FULL          │              │
│     │ Result: PASS                         │              │
│     └──────────────────────────────────────┘              │
│                                                             │
│  6. Skill delivery                                          │
│     • Agent requests delivery (kind:39221)                  │
│     • Skill provider delivers (NIP-44 encrypted)            │
│     • Agent decrypts (threshold ECDH)                       │
│     • Marketplace checks license before participating       │
│                                                             │
│  7. Hash verification                                       │
│     • Agent computes SHA-256 of decrypted SKILL.md          │
│     • Compares to manifest_hash in kind:33400               │
│     • If match: LOAD                                        │
│     • If mismatch: REJECT + violation label                 │
│                                                             │
│  8. Skill loaded into agent runtime                         │
│     • Capabilities registered                               │
│     • Runtime enforces declared permissions only            │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

### Workflow 2: Agent Executes Paid Job with Envelope

```
┌─────────────────────────────────────────────────────────────┐
│               OUTCOME-SCOPED PAYMENT FLOW                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. Agent wants to run NIP-90 inference job                 │
│                                                             │
│  2. Agent creates envelope request                          │
│     • scope: nip90:<job_hash>                               │
│     • max: 5000 sats                                        │
│     • duration: 1 hour                                      │
│                                                             │
│  3. Agent sends request to credit issuer                    │
│                                                             │
│  4. Issuer checks agent reputation                          │
│     • Past settlements (kind:39244)                         │
│     • Negative labels (kind:1985)                           │
│     • Wallet balance                                        │
│                                                             │
│  5. Issuer creates envelope (kind:39242)                    │
│     • Signs and publishes to relay                          │
│                                                             │
│  6. Agent constructs NIP-90 job request (kind:5050)         │
│     • Includes prompt, parameters                           │
│     • Signs with threshold signature                        │
│     • Agent share + marketplace share                       │
│                                                             │
│  7. Compute provider picks up job                           │
│     • Verifies envelope validity                            │
│     • Checks scope matches job ID                           │
│     • Checks not expired, not over cap                      │
│                                                             │
│  8. Provider runs inference                                 │
│     • Generates result                                      │
│     • Returns kind:6050 job result                          │
│                                                             │
│  9. Provider bills issuer                                   │
│     • Sends invoice: 3000 sats                              │
│     • References envelope ID                                │
│                                                             │
│  10. Issuer pays provider                                   │
│      • Lightning invoice paid                               │
│      • Settlement reference recorded                        │
│                                                             │
│  11. Issuer publishes settlement receipt (kind:39244)       │
│      • Links: envelope → scope → spend → outcome            │
│      • Includes repayment rail reference                    │
│                                                             │
│  12. Agent updates state                                    │
│      • Deducts 3000 sats from daily budget                  │
│      • Records job completion                               │
│      • Publishes trajectory events                          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

### Workflow 3: Automated Revocation on Negative Label

```
┌─────────────────────────────────────────────────────────────┐
│              AUTOMATED SAFETY ENFORCEMENT                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. Security researcher discovers vulnerability             │
│                                                             │
│  2. Researcher publishes negative label (kind:1985)         │
│     • L: skill-safety                                       │
│     • l: security-critical                                  │
│     • e: <skill_manifest_event_id>                          │
│     • reason: "Credential exfiltration"                     │
│                                                             │
│  3. Agent runtimes subscribed to trusted labelers           │
│     • Receive label via Nostr subscription                  │
│                                                             │
│  4. Runtime checks: is this skill loaded?                   │
│     ├─ YES → Suspend skill immediately                      │
│     │         Alert operator                                │
│     │         Unload from memory                            │
│     └─ NO → Add to deny list                                │
│              Reject future load attempts                    │
│                                                             │
│  5. Credit issuers subscribed to trusted labelers           │
│     • Receive same label                                    │
│                                                             │
│  6. Issuer checks: live envelopes for this skill?           │
│     • Query: envelopes with scope containing skill address  │
│                                                             │
│  7. For each live envelope:                                 │
│     • Update kind:39242 with status=revoked                 │
│     • Add revoke_reason tag                                 │
│     • Publish updated event                                 │
│                                                             │
│  8. Providers check envelope status before serving          │
│     • Fetch latest envelope event                           │
│     • If status=revoked → deny service                      │
│                                                             │
│  9. Result: skill blocked ecosystem-wide                    │
│     • No new loads                                          │
│     • No new payments                                       │
│     • Automated, no manual intervention                     │
│                                                             │
│  Time from label to enforcement: < 1 hour                   │
│  (depends on relay propagation + subscription latency)      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Code Examples

### Example 1: Verify Skill Manifest Hash (JavaScript)

```javascript
import { createHash } from 'crypto';
import { nip44 } from 'nostr-tools';

async function verifySkillManifest(manifest, encryptedPayload, agentPrivkey, marketplacePrivkey) {
  // 1. Check manifest signature
  const manifestValid = verifyEvent(manifest);
  if (!manifestValid) throw new Error('Invalid manifest signature');

  // 2. Check delegation chain (NIP-26)
  const delegationValid = verifyDelegationChain(manifest);
  if (!delegationValid) throw new Error('Invalid delegation chain');

  // 3. Check expiry
  const expiryTag = manifest.tags.find(t => t[0] === 'expiry');
  if (!expiryTag || parseInt(expiryTag[1]) < Math.floor(Date.now() / 1000)) {
    throw new Error('Manifest expired');
  }

  // 4. Decrypt skill payload (threshold ECDH)
  const sharedSecret = thresholdECDH(agentPrivkey, marketplacePrivkey);
  const skillContent = nip44.decrypt(encryptedPayload, sharedSecret);

  // 5. Compute hash
  const computedHash = createHash('sha256')
    .update(skillContent)
    .digest('hex');

  // 6. Verify against manifest
  const manifestHashTag = manifest.tags.find(t => t[0] === 'manifest_hash');
  const expectedHash = manifestHashTag[1].replace('sha256:', '');

  if (computedHash !== expectedHash) {
    throw new Error(`Hash mismatch: expected ${expectedHash}, got ${computedHash}`);
  }

  return {
    valid: true,
    content: skillContent,
    capabilities: manifest.tags
      .filter(t => t[0] === 'capability')
      .map(t => t[1])
  };
}
```

---

### Example 2: Enforce Capability at Runtime (Python)

```python
from typing import List, Optional

class AgentRuntime:
    def __init__(self):
        self.loaded_skills = {}
        self.violation_log = []

    def load_skill(self, skill_id: str, capabilities: List[str]):
        """Load skill with declared capabilities."""
        self.loaded_skills[skill_id] = {
            'capabilities': set(capabilities),
            'violation_count': 0
        }

    def check_capability(self, skill_id: str, requested_cap: str) -> bool:
        """Check if skill has requested capability."""
        if skill_id not in self.loaded_skills:
            return False

        skill = self.loaded_skills[skill_id]
        
        # Check if capability is declared
        if requested_cap not in skill['capabilities']:
            # Capability violation
            self.violation_log.append({
                'skill_id': skill_id,
                'requested': requested_cap,
                'declared': list(skill['capabilities']),
                'timestamp': time.time()
            })
            skill['violation_count'] += 1
            
            # Publish violation label (kind:1985)
            self.publish_violation_label(skill_id, requested_cap)
            
            # If repeated violations, suspend skill
            if skill['violation_count'] >= 3:
                self.suspend_skill(skill_id)
            
            return False
        
        return True

    def invoke_tool(self, skill_id: str, tool_name: str, capability: str, *args):
        """Invoke tool only if capability check passes."""
        if not self.check_capability(skill_id, capability):
            raise PermissionError(
                f"Skill {skill_id} attempted to invoke {tool_name} "
                f"but does not have {capability} capability"
            )
        
        # Capability check passed, invoke tool
        return self.execute_tool(tool_name, *args)
```

---

### Example 3: Create and Verify Envelope (TypeScript)

```typescript
import { Event, finishEvent, getEventHash } from 'nostr-tools';

interface Envelope {
  id: string;
  agent_pubkey: string;
  provider_pubkey: string;
  scope: { type: string; id: string };
  max_sats: number;
  exp: number;
  spent: number;
  status: 'offered' | 'accepted' | 'revoked' | 'spent' | 'settled' | 'defaulted';
}

class EnvelopeIssuer {
  private envelopes: Map<string, Envelope> = new Map();

  constructor(private readonly issuerPrivkey: string) {}

  createEnvelope(
    agent_pubkey: string,
    provider_pubkey: string,
    scope_type: string,
    scope_id: string,
    max_sats: number,
    duration_hours: number
  ): Event {
    const envelope_id = `envelope-${Date.now()}`;
    const exp = Math.floor(Date.now() / 1000) + duration_hours * 3600;

    const envelope: Envelope = {
      id: envelope_id,
      agent_pubkey,
      provider_pubkey,
      scope: { type: scope_type, id: scope_id },
      max_sats,
      exp,
      spent: 0,
      status: 'accepted'
    };

    this.envelopes.set(envelope_id, envelope);

    // Create Nostr event (kind:39242)
    const event = {
      kind: 39242,
      created_at: Math.floor(Date.now() / 1000),
      tags: [
        ['d', envelope_id],
        ['p', agent_pubkey],
        ['scope', scope_type, scope_id],
        ['max', max_sats.toString()],
        ['exp', exp.toString()],
        ['provider', provider_pubkey],
        ['status', 'accepted']
      ],
      content: ''
    };

    return finishEvent(event, this.issuerPrivkey);
  }

  async verifySpend(
    envelope_id: string,
    amount: number,
    scope_id: string
  ): Promise<{ valid: boolean; reason?: string }> {
    const envelope = this.envelopes.get(envelope_id);

    if (!envelope) {
      return { valid: false, reason: 'Envelope not found' };
    }

    if (envelope.status !== 'accepted') {
      return { valid: false, reason: `Envelope ${envelope.status}` };
    }

    const now = Math.floor(Date.now() / 1000);
    if (now > envelope.exp) {
      return { valid: false, reason: 'Envelope expired' };
    }

    if (envelope.spent + amount > envelope.max_sats) {
      return { valid: false, reason: 'Exceeds cap' };
    }

    // Verify scope matches
    if (scope_id !== envelope.scope.id) {
      return { valid: false, reason: 'Scope mismatch' };
    }

    // All checks passed
    envelope.spent += amount;
    return { valid: true };
  }

  publishSettlementReceipt(
    envelope_id: string,
    amount: number,
    job_result_id: string,
    repay_reference: string
  ): Event {
    const envelope = this.envelopes.get(envelope_id);
    
    return {
      kind: 39244,
      created_at: Math.floor(Date.now() / 1000),
      tags: [
        ['credit', envelope_id],
        ['p', envelope.agent_pubkey],
        ['issuer', 'npub1issuer...'],
        ['provider', envelope.provider_pubkey],
        ['scope', envelope.scope.type, envelope.scope.id],
        ['e', job_result_id, 'wss://relay.example.com', 'root'],
        ['repay', 'bolt11', repay_reference],
        ['status', 'settled']
      ],
      content: JSON.stringify({ schema: 1, spent_sats: amount, outcome: 'success' })
    };
  }
}
```

---

## Security Metrics Dashboard (Conceptual)

A production agent deployment should monitor these metrics:

```
┌─────────────────────────────────────────────────────────────┐
│              AGENT SECURITY METRICS DASHBOARD                │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Capability Enforcement                                     │
│  ├─ Unauthorized calls denied:      100% (42/42)           │
│  ├─ Capability drift incidents:     0                      │
│  └─ Violation labels published:     3                      │
│                                                             │
│  Skill Trust                                                │
│  ├─ Loaded skills:                  12                     │
│  ├─ Trust tier distribution:                               │
│  │   • Ultimate: 2                                         │
│  │   • Full: 5                                             │
│  │   • Marginal: 4                                         │
│  │   • None: 1 (monitoring only)                           │
│  └─ Revoked skills (last 30d):      1                      │
│                                                             │
│  Payment Security                                           │
│  ├─ Active envelopes:               5                      │
│  ├─ Total spending cap:             50,000 sats            │
│  ├─ Spent today:                    12,340 sats            │
│  ├─ Out-of-scope attempts:          0                      │
│  └─ Expired envelope blocks:        2                      │
│                                                             │
│  Threshold Signatures                                       │
│  ├─ Signature requests:             156                    │
│  ├─ Successful (2-of-3 met):        154 (98.7%)            │
│  ├─ Denied (policy check):          2 (1.3%)               │
│  └─ Average latency:                280ms                   │
│                                                             │
│  Incident Response                                          │
│  ├─ Safety labels received:         8                      │
│  ├─ Time to enforcement (avg):      42 minutes             │
│  ├─ Auto-revoked envelopes:         3                      │
│  └─ Manual reviews triggered:       1                      │
│                                                             │
│  Audit Trail                                                │
│  ├─ Trajectory events published:    1,247                  │
│  ├─ Settlement receipts:            23                     │
│  └─ Storage size:                   2.4 MB                 │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Implementation Checklist

For agent developers implementing NIP-SA/SKL/AC security controls:

### Phase 1: Identity and State
- [ ] Generate agent keypair
- [ ] Implement threshold key protection (2-of-3 or 3-of-5)
- [ ] Store Share 1 in secure enclave (iOS Keychain, Android Keystore, SGX)
- [ ] Register Share 2 with marketplace
- [ ] Store Share 3 with guardian or backup service
- [ ] Publish agent profile (kind:39200)
- [ ] Implement encrypted state storage (kind:39201)
- [ ] Set up state encryption/decryption with threshold ECDH

### Phase 2: Capability Enforcement
- [ ] Implement skill manifest verification (signature, delegation, expiry, hash)
- [ ] Build trust tier calculation (fetch attestations, apply weights)
- [ ] Implement runtime capability checks (default-deny)
- [ ] Add violation logging and label publishing
- [ ] Set up skill suspension mechanism
- [ ] Configure minimum trust tiers for each capability

### Phase 3: Payment Authorization
- [ ] Integrate with envelope issuer
- [ ] Implement envelope verification before spending
- [ ] Add scope checking (job ID, resource ID, skill address)
- [ ] Implement cap and expiry enforcement
- [ ] Set up spending anomaly alerts
- [ ] Publish settlement receipts after payments

### Phase 4: Audit and Monitoring
- [ ] Implement trajectory event publishing
- [ ] Set up real-time capability violation alerts
- [ ] Configure spending anomaly detection
- [ ] Subscribe to trusted labeler feeds
- [ ] Implement automated revocation triggers
- [ ] Build metrics dashboard

### Phase 5: Incident Response
- [ ] Document revocation procedures
- [ ] Set up guardian approval workflows
- [ ] Implement emergency key rotation
- [ ] Configure forensic log export
- [ ] Test incident response drills

---

## Conclusion

This technical appendix provides implementation-ready schemas, workflows, and code examples for the security controls described in the main RFI response. All protocols are based on open Nostr standards and can be implemented without vendor lock-in or centralized registries.

**Key implementation resources:**
- NIP-SA specification (full text)
- NIP-SKL specification (full text)
- NIP-AC specification (full text)
- Reference implementation: https://github.com/OpenAgentsInc
- Test vectors and compliance tools: [to be published]

**For NIST reviewers:**
These are not hypothetical proposals—they are actively being implemented and deployed in production agent systems. OpenAgents is available to provide additional technical details, reference implementations, and security audit results upon request.

---

**End of Technical Appendix**

*Supporting material for NIST RFI NIST-2025-0035 response*  
*Submitted by OpenAgents, March 4, 2026*
