# True Name Integration Profile (Satnam.pub Reference Implementation)

**Status:** Non-normative, ecosystem-specific profile layered on NIP-SKL / NIP-SA / NIP-AC.

This document specifies how a "True Name" trust layer (with Satnam.pub as the first
reference implementation) composes with the OpenAgents NIP triumvirate:

- **NIP-SKL** — Skill/Agent manifests, safety labels, assurance tiers.
- **NIP-SA** — Agent lifecycle, guardianship, security posture, audit and delegation.
- **NIP-AC** — Outcome-Scoped Credit Envelopes (OSCE), approvals, and hold periods.

The profile is **optional** and **non-normative**:

- It does *not* change SKL/SA/AC wire semantics.
- All integrations are expressed via optional tags and out-of-band behavior.
- Clients and relays MAY ignore these tags without breaking protocol correctness.

---

## 1. Scope

"True Name" refers to a class of systems that:

- Bind cryptographic identities (Nostr pubkeys, SKL manifests) to multi-method-verified
  identities (DNS / NIP-05, kind:0 metadata, PKARR DHT attestations, etc.).
- Maintain reputation and trust metrics for evaluators, agents, and issuers over time.
- Provide hierarchical governance and recovery for high-value keys (e.g., guardian
  and treasury keys).

**Satnam.pub** is the first full implementation of this profile. Other ecosystems MAY
follow the same tag conventions and semantics while using different backends.

---

## 2. Expected SKL Assurance and SA Guardian Profiles

### 2.1 SKL Assurance Tiers

NIP-SKL defines an optional `assurance_tier` tag on safety labels:

```json
["assurance_tier", "<tier>", "<evaluator_pubkey>", "<attestation_event_id>"]
```

Under this profile:

- Supported tiers:
  - `self-assessed`
  - `third-party-evaluated`
  - `red-team-tested`
- For `third-party-evaluated` and `red-team-tested`:
  - `<evaluator_pubkey>` SHOULD correspond to a True Name / Satnam **trust provider**.
  - `<attestation_event_id>` SHOULD reference an attestation that the trust layer
    indexes (e.g., evaluation report, test results, timestamp proof).

The True Name layer is responsible for maintaining evaluator registries, scores, and
attestation bundles. SKL itself remains agnostic.

### 2.2 Organizational Identity

NIP-SKL allows organizational identity binding via NIP-05-style identifiers, e.g.:

```json
["nip05", "agent-name@example.com"]
```

Under this profile, the True Name layer MAY:

- Verify DNS and `.well-known/nostr.json` for the claimed NIP-05.
- Cross-check with PKARR and kind:0 records.
- Expose a confidence score that AC underwriters and SA guardians can consume.

### 2.3 SA Guardian Profiles

NIP-SA defines guardian keys and thresholds that approve high-value operations.

Under this profile:

- Each SA guardian pubkey SHOULD correspond to a key governed by a True Name
  hierarchy (e.g., Satnam Guardian / Steward roles with multi-sig or FROST).
- The trust layer implements:
  - Role-based access control (Guardian / Steward / Adult / Offspring).
  - Hardware/NFC/WebAuthn MFA for high-risk approvals.
  - Emergency override and recovery workflows, with audit trails.

From NIP-SA's perspective, guardians are still just pubkeys; the True Name layer
adds governance semantics and logs around those keys.

---

## 3. How True Name Signals Feed AC and SA Decisions

### 3.1 AC Underwriting (OSCE Issuance and Limits)

When an AC issuer considers a Credit Intent or Credit Envelope, it MAY query the
True Name layer for:

- **Identity confidence** for the agent/skill pubkey:
  - NIP-05 alignment, PKARR records, kind:0 consistency, etc.
- **Assurance quality** from SKL safety labels:
  - Which `assurance_tier` labels exist?
  - Which evaluators issued them, and what are those evaluators' trust scores?
- **Reputation / risk** signals:
  - Prior defaults, revoked envelopes, or negative NIP-85 trust assertions.

These signals can influence:

- Whether to issue an OSCE at all.
- The `max` amount, `approval_threshold`, and `hold_period_secs` fields.
- Whether additional guardian approvals or tighter SKL permission grants are required.

### 3.2 SA Security Posture and Guardian Policy

NIP-SA allows a `security_posture` declaration in the agent profile content. Under
this profile, fields like `hijacking_resistance_tier` and `evaluation_ref` MAY point
at SKL/Satnam evaluations coming from the True Name layer.

Guardians and operators MAY define local policies such as:

- "Disallow autonomous high-risk actions unless the agent has at least one
  `third-party-evaluated` or `red-team-tested` label from a trusted provider."
- "Require multi-guardian approval when evaluations are stale or missing."

---

## 4. Optional Tags for Referencing True Name Artifacts

The following tags are **optional** and safe to ignore. They carry hints for
clients that implement this integration profile.

### 4.1 SKL Events (Manifests, Safety Labels)

Suggested optional tags:

```json
["true_name_provider", "<trust_provider_id>"],
["true_name_score", "<normalized_score>"],
["true_name_profile", "satnam:family/<family_id>"]
```

### 4.2 SA Events (Guardian, Delegation, Audit)

On guardian, delegation, or audit events, implementations MAY include:

```json
["true_name_role", "guardian"],           // or "steward", "adult"
["true_name_federation", "satnam:fed/<id>"],
["true_name_policy", "satnam:policy/<policy_id>"],
["true_name_context", "satnam:session/<session_id>"]
```

These tags allow correlation between Nostr-level events and the True Name
hierarchy, policies, and audit dashboards.

### 4.3 AC Events (Credit Intent, Envelope, Spend)

On AC credit intents, envelopes, and spend authorizations, implementations MAY
include:

```json
["true_name_trust_provider", "<trust_provider_id>"],
["true_name_trust_score", "<normalized_score>"],
["true_name_risk_band", "low"],   // or "medium", "high"
```

These give underwriters, providers, and monitoring tools a compact view of how
True Name / Satnam trust signals influenced an OSCE.

---

## 5. Non-Normative Nature and Extensibility

This profile deliberately keeps all True Name integrations **out of band**:

- SKL/SA/AC remain the authoritative protocol specifications.
- All `true_name_*` tags are OPTIONAL and may be ignored by generic Nostr relays
  and clients.
- Other ecosystems MAY adopt the same tag patterns with different backends, as
  long as they preserve optionality and protocol compatibility.

Satnam.pub is the first full True Name implementation. Future systems MAY
implement their own True Name layers, reusing these conventions to provide
richer identity, reputation, and governance semantics around the NIP triumvirate.
