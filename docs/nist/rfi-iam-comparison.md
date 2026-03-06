# Enterprise IAM vs. Open Protocol Comparison for AI Agent Systems
## Supporting Material for NIST RFI NIST-2025-0035 Response

**Submitter:** OpenAgents  
**Date:** March 4, 2026

> Archival note: this document preserves submission materials prepared for NIST RFI `NIST-2025-0035` on March 4, 2026. It is not the normative source of truth for current OpenAgents product or protocol behavior.

---

## Executive Summary

This document compares two valid compliance paths for AI agent security: traditional enterprise Identity and Access Management (IAM) systems versus open cryptographic protocols. **Both approaches can meet NIST's security objectives**, but they differ in architecture, cost, and applicability to different deployment scenarios.

NIST guidance should explicitly recognize both paths to avoid de facto exclusion of open-source agents, small developers, and cross-domain agent ecosystems.

---

## Architectural Comparison

### Identity Model

| Aspect | Enterprise IAM | Open Protocols (NIP-SA/SKL/AC) |
|--------|----------------|--------------------------------|
| **Identity authority** | Centralized directory (LDAP, Active Directory, Cloud IAM) | Self-sovereign keypairs (npub/nsec, HD-derived) |
| **Enrollment process** | Administrator creates account, assigns credentials | Agent generates own keypair deterministically from seed |
| **Identity portability** | Tied to organization; lost when leaving | Portable across deployments, vendors, time |
| **Multi-domain transactions** | Requires federation (SAML, OAuth, trust agreements) | Native; no pre-existing trust relationship needed |
| **Identity continuity** | Depends on org retaining records | Cryptographic identity persists regardless of org |
| **Revocation mechanism** | Admin disables account in directory | Publish signed revocation event (kind:5, NIP-09) |
| **Recovery** | Help desk, admin reset | Seed phrase (BIP-39) or threshold reconstruction |
| **Proof of identity** | Session token issued by IdP | Cryptographic signature over events |

**Key difference:** Enterprise IAM assumes administrative control over identities. Open protocols assume agents control their own identities.

---

### Authorization Model

| Aspect | Enterprise IAM | Open Protocols (NIP-SA/SKL/AC) |
|--------|----------------|--------------------------------|
| **Permission model** | RBAC (roles) or ABAC (attributes) defined by admin | Capability-based: manifests declare required permissions |
| **Permission source** | Central policy store (database, directory) | Signed manifests (kind:33400) with capability tags |
| **Permission enforcement** | Centralized policy decision point (PDP) | Runtime checks declared capabilities, default-deny |
| **Permission updates** | Admin updates policy in central system | Author publishes new manifest version, agents verify signature |
| **Cross-domain authorization** | Requires pre-configured trust (federation agreements) | Agents verify manifest signatures, check attestations |
| **Delegation** | Admin delegates roles/permissions to users | Cryptographic delegation chains (NIP-26) |
| **Revocation speed** | Admin disables in central system; propagation depends on token TTL | Publish revocation event; agents subscribe and enforce immediately |
| **Audit trail** | Central logs (may be vendor-locked) | Signed events on open relays (user controls where stored) |

**Key difference:** Enterprise IAM centralizes authorization decisions. Open protocols distribute authorization verification to each party.

---

### Trust and Attestation

| Aspect | Enterprise IAM | Open Protocols (NIP-SA/SKL/AC) |
|--------|----------------|--------------------------------|
| **Trust model** | Hierarchical: trust the CA, directory, IdP | Web-of-trust: attestations from trusted labelers (kind:1985) |
| **Who can attest?** | Certificate authorities, org admins | Anyone; agents choose which labelers to trust |
| **Attestation format** | X.509 certificates, SAML assertions | Signed Nostr events with zap-weighted reputation |
| **Trust tiers** | Binary (valid/invalid cert) | Graduated (none, marginal, full, ultimate) |
| **Negative attestations** | Certificate revocation lists (CRLs), OCSP | Safety labels (kind:1985) with security-critical flag |
| **Attestation discovery** | Query CRL/OCSP endpoint | Subscribe to trusted labelers on Nostr relays |
| **Economic signaling** | None (certs are free or flat fee) | Zap amounts weighted in trust calculation |
| **Revocation propagation** | CRL distribution, OCSP responses | Event publication to subscribed relays (push model) |

**Key difference:** Enterprise IAM relies on hierarchical certificate authorities. Open protocols use distributed web-of-trust with economic signaling.

---

### Payment Authorization (Agent Wallets)

| Aspect | Enterprise IAM | Open Protocols (NIP-SA/SKL/AC) |
|--------|----------------|--------------------------------|
| **Payment model** | Corporate card, API keys, pre-funded accounts | Outcome-scoped credit envelopes (kind:39242) |
| **Authorization granularity** | Per-transaction approval or blanket limits | Per-scope (job, resource, skill) with cap + expiry |
| **Spending enforcement** | Payment gateway checks account balance | Issuer checks envelope status, cap, expiry, scope |
| **Auditability** | Transaction logs in payment system | Settlement receipts (kind:39244) with outcome artifact links |
| **Refunds/disputes** | Chargeback process via payment network | Cryptographic proof of payment + outcome; arbitration via signed evidence |
| **Multi-party payments** | Requires payment network coordination | Native: agent, issuer, provider, verifier via envelopes |
| **Cross-domain payments** | Requires payment gateway integrations | Native: Lightning, Cashu, Fedimint rails |
| **Budget constraints** | Set in payment system or approval workflow | Encoded in envelope (max, exp, scope tags) |

**Key difference:** Enterprise IAM treats payments as external (payment gateway). Open protocols treat payments as first-class authorization capability.

---

## Deployment Scenario Comparison

### Scenario 1: Enterprise Internal Agent

**Use case:** Agent assists employees within single organization (e.g., customer support bot, internal research assistant).

| Aspect | Enterprise IAM | Open Protocols |
|--------|----------------|----------------|
| **Applicability** | ✅ Excellent fit | ✅ Works, but extra features unused |
| **Identity management** | Use existing AD/LDAP | Generate keypair, publish profile |
| **Access control** | Use existing RBAC policies | Publish capability manifests |
| **Audit** | Central SIEM | Trajectory events to org relay |
| **Payments** | Corporate card/account | Envelopes scoped to internal jobs |
| **Cost** | Marginal (infrastructure exists) | Marginal (minimal infra needed) |
| **Vendor lock-in** | High (tied to IAM vendor) | Low (open protocols) |

**Recommendation:** Either approach works. Enterprise IAM may be simpler if infrastructure already exists.

---

### Scenario 2: Open-Source Agent (Personal Use)

**Use case:** Individual runs open-source agent on personal device (e.g., research assistant, content curator).

| Aspect | Enterprise IAM | Open Protocols |
|--------|----------------|----------------|
| **Applicability** | ❌ Does not fit | ✅ Excellent fit |
| **Identity management** | Requires enrollment in someone's directory (whose?) | Generate keypair from seed phrase |
| **Access control** | Requires policy admin (who administers?) | Load skills with capability manifests |
| **Audit** | Requires SIEM subscription (expensive for individual) | Publish trajectories to free relay |
| **Payments** | Requires corporate account or pre-funded wallet | Obtain envelopes from issuer as needed |
| **Cost** | Prohibitive (IAM licensing) | Minimal (relay + compute costs) |
| **Vendor lock-in** | High | None |

**Recommendation:** Open protocols are the only practical option. Enterprise IAM not designed for individual users.

---

### Scenario 3: Cross-Organization Agent Marketplace

**Use case:** Agents from different operators interact in marketplace (e.g., agent buys skill from marketplace, pays compute provider, receives service).

| Aspect | Enterprise IAM | Open Protocols |
|--------|----------------|----------------|
| **Applicability** | ⚠️ Possible with federation | ✅ Native use case |
| **Identity management** | Requires pre-configured SAML/OAuth federation between all orgs | Each agent has keypair; no pre-configuration needed |
| **Access control** | Requires trust agreements, federated policy decisions | Agent verifies skill manifest signature; checks attestations |
| **Audit** | Each org keeps separate logs; correlation difficult | Agents publish trajectories; references link events |
| **Payments** | Requires payment gateway integrations between orgs | Native: envelopes, Lightning/Cashu/Fedimint rails |
| **Cost** | High (federation setup, legal agreements, integration) | Low (relays, compute) |
| **Vendor lock-in** | High (each org tied to their IAM vendor) | None |

**Recommendation:** Open protocols dramatically simpler for cross-organization interactions. Enterprise IAM federation complex and expensive.

---

### Scenario 4: SaaS Agent Platform (Multi-Tenant)

**Use case:** Cloud provider hosts agents for many customers (e.g., AI agent hosting service).

| Aspect | Enterprise IAM | Open Protocols |
|--------|----------------|----------------|
| **Applicability** | ✅ Good fit | ✅ Good fit |
| **Identity management** | Provider creates accounts for customer agents in provider's directory | Customers generate keypairs; provider hosts runtime |
| **Access control** | Provider enforces RBAC policies | Runtime enforces capability manifests |
| **Audit** | Provider logs in central SIEM | Agents publish trajectories to customer-chosen relays |
| **Payments** | Provider manages billing, customer funds agents | Customers issue envelopes; agents spend against envelopes |
| **Cost** | Marginal (provider already has IAM) | Marginal (minimal infra) |
| **Customer lock-in** | High (identity tied to provider) | Low (customer controls keypairs, can migrate) |
| **Transparency** | Customer trusts provider logs | Customer can verify agent actions via signed events |

**Recommendation:** Both work. Open protocols provide better customer data sovereignty and portability.

---

## Security Properties Comparison

### Key Extraction / Insider Threat

| Property | Enterprise IAM | Open Protocols (Threshold Keys) |
|----------|----------------|--------------------------------|
| **Admin can extract agent key?** | Yes (admin has full access) | No (key split across threshold shares) |
| **Operator can extract agent key?** | Depends on key storage | No (Share 1 in secure enclave; Share 2 with marketplace) |
| **Marketplace can sign alone?** | N/A | No (needs agent share) |
| **Agent can sign alone?** | N/A | No (needs marketplace or guardian share) |
| **Mitigation for compromised operator** | Rely on admin detecting misuse | Marketplace refuses to participate if license invalid |

**Key difference:** Enterprise IAM trusts admins. Open protocols use threshold cryptography to limit trust.

---

### Revocation Speed

| Scenario | Enterprise IAM | Open Protocols |
|----------|----------------|----------------|
| **Revoke compromised agent** | Admin disables in directory; tokens invalid on next refresh (minutes to hours) | Publish revocation event (kind:5); agents subscribed to relay see immediately (<1 minute) |
| **Revoke malicious skill** | Admin updates policy; agents check on next policy refresh (minutes to hours) | Trusted labeler publishes negative label (kind:1985); agents subscribed see immediately (<1 minute) |
| **Revoke payment authorization** | Admin disables payment account; pending transactions may complete | Issuer updates envelope status=revoked (kind:39242); providers check before serving |

**Key difference:** Open protocols push revocations to subscribers immediately. Enterprise IAM relies on polling intervals.

---

### Audit Trail Ownership

| Aspect | Enterprise IAM | Open Protocols |
|--------|----------------|----------------|
| **Who owns logs?** | Organization (employer, platform provider) | Agent operator (can publish to own relays) |
| **Can logs be tampered?** | Admin has full access to log storage | Events cryptographically signed; tampering detectable |
| **Can logs be deleted?** | Admin can delete | Relays can delete, but operator can republish from backup |
| **Forensic analysis** | Requires access to org's SIEM | Anyone with relay access can verify signed events |
| **Dispute resolution** | Org provides logs (may be disputed) | Cryptographic proofs (signatures, payment preimages) |

**Key difference:** Enterprise IAM centralizes log ownership. Open protocols give agents sovereignty over their audit trails.

---

## Cost Comparison (Rough Order of Magnitude)

### Setup Costs

| Component | Enterprise IAM | Open Protocols |
|-----------|----------------|----------------|
| **Identity infrastructure** | $50k–$500k (IAM platform, directory, integration) | $0–$5k (relay setup, optional) |
| **Policy management** | Included in IAM platform | $0–$10k (manifest tooling, optional) |
| **Audit infrastructure** | $20k–$200k (SIEM, log storage) | $0–$5k (relay storage, optional) |
| **Federation setup** | $10k–$100k per partner (legal, technical integration) | $0 (no pre-coordination needed) |
| **Total setup** | $80k–$800k+ | $0–$20k |

---

### Ongoing Costs (per agent, per year)

| Component | Enterprise IAM | Open Protocols |
|-----------|----------------|----------------|
| **Identity licensing** | $5–$50 per agent per year | $0 |
| **Policy management** | Included | $0 |
| **Audit storage** | $1–$10 per agent per year (SIEM ingestion) | $0.10–$1 per agent per year (relay storage) |
| **Payment processing** | 2–3% payment gateway fees | Lightning: <0.5%, Cashu: ~0%, Fedimint: ~0% |
| **Total ongoing** | $6–$60+ per agent per year | $0.10–$2 per agent per year |

**Key difference:** Enterprise IAM has significant licensing costs. Open protocols have minimal infrastructure costs.

---

### Break-Even Analysis

**For small-scale deployments (1–100 agents):**
- Enterprise IAM: $80k–$850k over 5 years
- Open Protocols: $50–$1,000 over 5 years
- **Savings: 99%+ with open protocols**

**For large enterprise (10,000+ agents):**
- Enterprise IAM: $500k (existing infra) + $60k/year = $800k over 5 years
- Open Protocols: $20k (setup) + $20k/year = $120k over 5 years
- **Savings: 85% with open protocols**

**For cross-organization marketplace (1,000 agents from 100 orgs):**
- Enterprise IAM: $10M+ (federation setup for 100 org pairs)
- Open Protocols: $100k (marketplace infra)
- **Savings: 99% with open protocols**

---

## Applicability by Agent Deployment Type

| Deployment Type | Enterprise IAM | Open Protocols | Recommended |
|-----------------|----------------|----------------|-------------|
| **Enterprise internal (single-org)** | ✅ Excellent | ✅ Good | Either (prefer existing infra) |
| **Open-source personal agents** | ❌ Does not fit | ✅ Excellent | Open protocols only |
| **Cross-org marketplaces** | ⚠️ Possible but expensive | ✅ Excellent | Open protocols |
| **Multi-tenant SaaS platforms** | ✅ Good | ✅ Excellent | Open protocols (better portability) |
| **Embedded agents (IoT, edge)** | ⚠️ Heavyweight | ✅ Lightweight | Open protocols |
| **Agent-to-agent coordination** | ⚠️ Requires federation | ✅ Native | Open protocols |
| **High-security government/defense** | ✅ Excellent (with HSMs) | ✅ Excellent (with threshold + TEE) | Either (both support high-assurance) |

---

## Compliance Pathway Recommendations for NIST

### Option A: Dual Recognition (RECOMMENDED)

**Recommendation:** NIST guidance should explicitly state:

> "Organizations MAY implement agent identity and authorization using either:
> 
> (a) **Enterprise IAM approach**: Centralized directory (LDAP, Active Directory, Cloud IAM), RBAC/ABAC policies, centralized audit logs, payment gateway integration.
> 
> (b) **Open protocol approach**: Self-sovereign cryptographic identity (keypairs), capability-based authorization (signed manifests), distributed audit trails (signed event logs), native payment authorization (scope-cap-expiry envelopes).
> 
> Both approaches can meet the functional security requirements if implemented correctly. Organizations should choose based on deployment scenario, cost, and architectural fit."

**Benefits:**
- Preserves innovation pathways for open-source agents
- Avoids vendor lock-in
- Enables cross-domain agent ecosystems
- Recognizes architectural diversity

---

### Option B: Enterprise-Only (NOT RECOMMENDED)

**If NIST only recognizes enterprise IAM:**

**Consequences:**
- Open-source agents effectively excluded (individuals cannot afford IAM licensing)
- Cross-organization marketplaces prohibitively expensive (federation setup)
- Vendor lock-in increases (compliance tied to specific IAM platforms)
- Innovation concentration (only large vendors can participate)

**Analogy:** Similar to if NIST cybersecurity guidance in 1990s only recognized proprietary encryption, excluding open standards like TLS/SSL. Would have significantly harmed internet security and interoperability.

---

### Option C: Open-Protocol-Only (NOT RECOMMENDED)

**If NIST only recognizes open protocols:**

**Consequences:**
- Large enterprises with existing IAM investments forced to migrate (expensive)
- May face resistance from established security teams
- Could slow adoption of AI agents in regulated industries

**Balance is needed:** Both approaches valid for different contexts.

---

## Implementation Checklist by Approach

### If Using Enterprise IAM

Security checklist for NIST compliance:

- [ ] **Identity:** Agent identities managed in centralized directory with unique identifiers
- [ ] **Authentication:** Strong authentication (cert-based, MFA) for agent access
- [ ] **Authorization:** RBAC/ABAC policies define tool and resource access
- [ ] **Capability enforcement:** Runtime checks policies before tool invocation
- [ ] **Audit:** Agent actions logged to central SIEM with tamper-evident storage
- [ ] **Payment authorization:** Payment limits and approval workflows in payment gateway
- [ ] **Revocation:** Admin can disable agent access in <1 hour
- [ ] **Incident response:** Playbooks for compromised agents, malicious skills
- [ ] **Documentation:** Architecture diagrams, policy definitions, audit procedures

---

### If Using Open Protocols

Security checklist for NIST compliance:

- [ ] **Identity:** Agent generates HD keypair, publishes profile (kind:39200)
- [ ] **Key protection:** Threshold keys (2-of-3) with Share 1 in secure enclave
- [ ] **Authorization:** Skills publish capability manifests (kind:33400), runtime enforces
- [ ] **Trust:** Agent checks skill attestations (kind:1985) from trusted labelers
- [ ] **Hash verification:** Agent verifies manifest_hash matches delivered payload
- [ ] **Payment authorization:** Agent uses outcome-scoped envelopes (kind:39242)
- [ ] **Audit:** Agent publishes trajectory events (kind:39230–39231) to relay
- [ ] **Revocation:** Subscribe to trusted labelers; enforce negative labels immediately
- [ ] **Incident response:** Automated revocation triggers, forensic log export
- [ ] **Documentation:** Protocol specifications, event schemas, relay configuration

---

## Feature Parity Matrix

Both approaches can achieve functional equivalence:

| Security Requirement | Enterprise IAM Implementation | Open Protocol Implementation |
|----------------------|------------------------------|------------------------------|
| **Persistent identity** | User account in directory | Keypair (npub/nsec) |
| **Identity verification** | Certificate from CA | Signature over events |
| **Least privilege** | RBAC roles define minimal permissions | Capability manifest declares minimal permissions |
| **Authorization enforcement** | PDP checks policy before allowing action | Runtime checks capability before allowing tool call |
| **Audit trail** | Logs to SIEM | Signed events to relay |
| **Tamper evidence** | SIEM log integrity checks | Cryptographic signatures |
| **Revocation** | Admin disables account | Publish revocation event |
| **Payment authorization** | Budget limits in payment gateway | Envelope with scope + cap + expiry |
| **Multi-party coordination** | Federation agreements + trust setup | Verify signatures + check attestations |
| **Incident response** | Admin intervention + policy updates | Automated safety label enforcement |

**Key insight:** Functional requirements can be met by both architectures. The difference is centralized vs. distributed trust model.

---

## Summary Recommendations for NIST

1. **Explicitly recognize both enterprise IAM and open cryptographic protocols as valid compliance paths** in forthcoming guidance.

2. **Define functional security requirements** (persistent identity, least privilege, tamper-evident audit, revocation, etc.) rather than prescribing specific technologies.

3. **Provide architectural guidance for each approach** with reference implementations and checklists.

4. **Acknowledge different deployment scenarios** (enterprise internal vs. open-source personal vs. cross-org marketplace) and recommend approach based on fit.

5. **Avoid mandating centralized registries or directories** as the only acceptable identity model (would exclude open-source agents and cross-domain ecosystems).

6. **Emphasize cryptographic verification and audit trails** as key security properties, achievable by either approach.

7. **Support interoperability**: Agents using enterprise IAM should be able to transact with agents using open protocols (e.g., via signed messages, standardized event formats).

---

## Conclusion

Enterprise IAM and open cryptographic protocols are **complementary, not competing** approaches. Both can meet NIST's security objectives for AI agent systems. The optimal choice depends on deployment context:

- **Enterprise internal agents:** Either works; prefer existing infrastructure
- **Open-source personal agents:** Open protocols only practical option
- **Cross-organization marketplaces:** Open protocols dramatically simpler
- **High-security deployments:** Both can achieve high assurance with appropriate controls

NIST guidance should preserve architectural diversity by explicitly recognizing both paths, enabling innovation while maintaining security standards.

---

**End of Comparison Document**

*Supporting material for NIST RFI NIST-2025-0035 response*  
*Submitted by OpenAgents, March 4, 2026*
