# NIST RFI NIST-2025-0035: Executive Summary
## Open Protocol Approach to AI Agent Security
**Submitter:** OpenAgents  
**Date:** March 4, 2026

> Archival note: this document preserves submission materials prepared for NIST RFI `NIST-2025-0035` on March 4, 2026. It is not the normative source of truth for current OpenAgents product or protocol behavior.

---

## Core Message

AI agent security failures are fundamentally **identity and authorization failures**—not model errors. As agents gain tool access, credentials, and payment authority, traditional enterprise IAM approaches face two critical limitations:

1. **They assume centralized administrative domains** that don't exist in open agent ecosystems where agents transact across marketplaces, compute providers, and payment rails
2. **They cannot prevent insider/operator extraction** of agent keys, breaking accountability chains

OpenAgents urges NIST to **explicitly recognize open, cryptographic, decentralized identity and authorization protocols as valid compliance paths** alongside enterprise patterns, preserving innovation pathways for open-source developers and preventing de facto regulatory capture by centralized identity providers.

---

## Key Technical Proposals

### 1. Treat Agents as Cryptographic Principals
**NIP-SA (Sovereign Agents)** defines agents with persistent Nostr identities that publish:
- Profile, state, goals (`kind:39200–39203`)
- Execution audit logs ("trajectories," `kind:39230–39231`)
- Threshold key protection (FROST/FROSTR) preventing operator extraction

**Security property:** Identity continuity enables auditing and accountability across deployments without centralized registries.

### 2. Capability Contracts for Least Privilege
**NIP-SKL (Skill Registry)** requires skills to publish signed manifests (`kind:33400`) declaring capability flags:
- `shell:exec`, `credentials:read`, `payment:lightning`, `payment:cashu:melt`, etc.
- Runtimes **MUST deny undeclared permissions** at execution time
- High-risk capabilities require higher trust tiers and attestations (`kind:1985`, NIP-32)
- Violations trigger ecosystem-wide safety labels for automated blocking

**Addresses:** Indirect prompt injection (blast radius bounded by declared capabilities), supply-chain attacks (hash-committed payloads + revocation), tool misuse.

### 3. Outcome-Scoped Authorization for Payments
**NIP-AC (Agent Credit)** replaces free-floating agent wallets with bounded "credit envelopes":
- `scope` binding (e.g., `nip90:<job_hash>`, `l402:<resource_id>`)
- Hard sat `max` cap and `exp` expiry timestamp
- Auditable settlement receipts (`kind:39244`) linking scope → payment → outcome
- Guardian co-approval thresholds for high-risk spends

**Security property:** Payment authorization becomes measurable, scoped capability; compromised agents cannot drain unrestricted funds.

---

## Answers to NIST's Five Categories

| Category | Open Protocol Solution |
|----------|------------------------|
| **1. Unique Threats** | Indirect prompt injection = capability escalation; mitigate with manifest-declared permissions + runtime enforcement |
| **2. Security Practices** | Threshold keys (anti-extraction), capability contracts (least privilege), safety labels (automated revocation) |
| **3. Measuring Security** | Rate of denied unauthorized calls, capability drift detection, time-to-revoke metrics, cryptographic audit trails |
| **4. Deployment Controls** | Trust-gated skill loading (default-deny), hash verification, scope-cap-expiry envelopes, safety label triggers |
| **5. Gaps** | Enterprise IAM assumes centralized domains; agents need open, cryptographic alternatives for cross-domain transactions |

---

## Concrete Use Case

**Agent runs paid inference job:**
1. Agent has persistent Nostr identity, publishes state/goals/audit logs (NIP-SA)
2. Agent checks skill manifest: capabilities, trust tier, attestations, hash-committed payload (NIP-SKL)
3. Agent obtains outcome-scoped credit envelope for specific job with sat cap + expiry (NIP-AC)
4. If safety label later marks skill malicious → envelope revoked, future calls blocked (automated enforcement)

**Result:** End-to-end identity + authorization + audit + payments without centralized registry.

---

## Specific NIST Recommendations

1. **Define agents as software principals** requiring identity/authorization controls distinct from user sessions
2. **Require hard authorization boundaries** at tool/payment rails (default-deny, capability enumeration)
3. **Explicitly recognize open protocol alternatives** for identity/delegation/capabilities to avoid regulatory capture
4. **Promote scope-cap-expiry authorization** as best practice for autonomous actions
5. **Standardize safety signaling loops:** signed labels → automated revocation (not just alerts)

---

## Why This Matters

If NIST guidance assumes centralized identity registries and enterprise IAM as the only valid approach:
- Open-source agents and small developers are effectively excluded (compliance becomes cost-prohibitive)
- Security innovation concentrates in large vendors
- Agent ecosystems fragment along administrative boundaries

**Open cryptographic protocols enable security at scale** without gatekeepers, consistent with NIST's historical support for open standards and interoperability.

---

**Contact:** OpenAgents is available to provide implementation examples, test vectors, and open-source reference code.

**Supporting materials in this directory:**
- NIP-AC: Agent Credit (Outcome-Scoped Credit Envelopes)
- NIP-SKL: Agent Skill Identity, Manifest, and Trust Registry
- Full response and technical appendix
