NIP-SKL
=======

Agent Skill Registry
--------------------

`draft` `optional`

This NIP defines the registry and trust substrate for agent skills on Nostr.

## Abstract

Core responsibilities are:

- Canonical skill identity
- Canonical skill manifest (`kind:33400`)
- Append-only version log (`kind:33401`)
- Attestation/trust signals (NIP-32 `kind:1985`)
- Publisher-origin revocation semantics (NIP-09 `kind:5`)
- Canonical discovery baseline

NIP-SKL does not standardize ecosystem-specific migration programs, proprietary gateway formats, or payment rail business logic. Those belong in optional profiles layered on top of SKL core.

NIP-SA remains the fulfillment/runtime layer (license, delivery, execution lifecycle). NIP-AC remains the credit layer.

---

## Scope And Layering

### SKL Core (normative)

- Defines what a skill is at protocol level.
- Defines how agents evaluate trust for loading.
- Defines baseline discovery that does not depend on a specific marketplace or ecosystem.

### Out Of Scope For Core (non-normative profiles)

- BreezClaw/ClaWHub/OpenClaw migration bridges
- L402 and gateway vendor contracts
- Cashu/Fedimint commercial policy details
- Indexer implementation details and rollout plans

---

## Kinds

This NIP introduces:

| Kind | Type | Description |
|------|------|-------------|
| 33400 | Addressable (parameterized replaceable) | Skill Manifest |
| 33401 | Regular | Skill Version Log |

This NIP reuses:

| Kind | NIP | Role |
|------|-----|------|
| 1985 | NIP-32 | Skill attestations / safety labels |
| 5 | NIP-09 | Publisher-origin revocation |
| 30402 | NIP-99 | Optional listing surface |

Optional discovery profile:

| Kind | NIP | Role |
|------|-----|------|
| 5390 / 6390 | NIP-90 | Skill search request/result profile |

---

## 1. Skill Identity

### 1.1 Canonical Skill Address

The canonical address of a skill manifest is:

```text
33400:<skill_pubkey>:<d-tag>
```

This address identifies the current manifest head for that skill identity.

### 1.2 Versioned Skill Scope ID

For AC interoperability and pinned version references:

```text
skill_scope_id = 33400:<skill_pubkey>:<d-tag>:<version>
```

### 1.3 Key Derivation

Implementations MAY derive skill keys from a NIP-06 seed using a deterministic path extension. A commonly used pattern is:

```text
m/44'/1237'/agent_account'/skill_type'/skill_index'
```

This derivation pattern is recommended for interoperability but not required for protocol validity.

### 1.4 Signing Profiles

SKL core supports two signing profiles.

#### Profile A: Direct Publisher (required support)

- `kind:33400` and `kind:33401` are signed directly by the skill publisher key.
- This is the minimum interoperable profile.

#### Profile B: Delegated Publisher (optional)

- Implementations MAY use NIP-26 delegated signing chains for operational separation.
- Because NIP-26 is marked unrecommended in the canonical NIPs repository, SKL does not require it for baseline interoperability.

If delegated signing is used, clients SHOULD verify delegation constraints and expiry before trust elevation.

---

## 2. Skill Manifest (`kind:33400`)

Addressable event keyed by `d`.

### 2.1 Required Tags

A valid `kind:33400` manifest MUST include:

- `d`: stable skill slug/identifier
- `name`: human-readable name
- `version`: semantic version string
- `description`: short summary
- `manifest_hash`: SHA-256 of canonical SKILL payload bytes
- `capability`: repeated capability declarations, or a single `none`
- `expiry`: unix timestamp after which the manifest is stale
- `t`: include `agent-skill`

### 2.2 Recommended Tags

- `author_npub`: canonical author/maintainer identity
- `p`: author pubkey with relay hint
- `skill_scope_id`: `33400:<skill_pubkey>:<d-tag>:<version>`
- `v`: previous manifest event id for explicit upgrade chain
- `t`: extra keyword tags

### 2.3 Example

```json
{
  "kind": 33400,
  "pubkey": "<skill_pubkey>",
  "created_at": 1740400000,
  "tags": [
    ["d", "research-assistant"],
    ["name", "Research Assistant"],
    ["version", "1.4.2"],
    ["description", "Summarize and structure technical research."],
    ["author_npub", "<author_npub>"],
    ["capability", "http:outbound"],
    ["capability", "filesystem:read"],
    ["manifest_hash", "<sha256_hex>"],
    ["expiry", "1756000000"],
    ["skill_scope_id", "33400:<skill_pubkey>:research-assistant:1.4.2"],
    ["t", "agent-skill"],
    ["t", "research"]
  ],
  "content": "Minor prompt hardening and schema cleanup"
}
```

### 2.4 Canonical Payload Hashing

`manifest_hash` MUST be computed as SHA-256 over canonical SKILL payload bytes:

1. UTF-8
2. no BOM
3. LF (`\n`) line endings
4. exact bytes intended for runtime load

Runtimes MUST verify payload hash equality before loading a skill.

### 2.5 Capability Declaration Rule

Runtimes MUST NOT grant undeclared permissions. If undeclared capabilities are observed at runtime, clients SHOULD emit a NIP-32 safety label and treat the skill as untrusted pending operator policy.

---

## 3. Skill Version Log (`kind:33401`)

`kind:33401` is a regular append-only changelog entry for a given skill identity.

### 3.1 Required Tags

- `d`: same skill identifier as manifest
- `version`: current semantic version
- `manifest_event`: event id of the referenced `kind:33400`
- `manifest_hash`: hash at that version
- `change_type`: `added|changed|fixed|deprecated|security`

### 3.2 Example

```json
{
  "kind": 33401,
  "pubkey": "<skill_pubkey>",
  "created_at": 1740400500,
  "tags": [
    ["d", "research-assistant"],
    ["version", "1.4.2"],
    ["prev_version", "1.4.1"],
    ["manifest_event", "<event_id_33400>"],
    ["manifest_hash", "<sha256_hex>"],
    ["change_type", "security"]
  ],
  "content": "Patched prompt-injection vector in parser path"
}
```

---

## 4. Trust And Attestations (NIP-32)

### 4.1 Attestation Event

Skill trust signals SHOULD be emitted as `kind:1985` labels referencing either:

- skill pubkey (`p`)
- specific manifest event (`e`)
- skill address (`a`, preferred for addressable targeting)

### 4.2 Suggested Label Namespace

Use `L=skill-security` and `l` values such as:

- `audit-passed`
- `scan-clean`
- `capabilities-verified`
- `delivery-hash-verified`
- `malicious-confirmed`
- `prompt-injection`
- `credential-exfil`
- `capability-violation`

Label interpretation and quorum policy are local runtime policy.

---

## 5. Revocation And Safety

### 5.1 Publisher-Origin Revocation (`kind:5`, NIP-09)

Revocation by deletion request MUST follow NIP-09 semantics:

- A `kind:5` deletion request only has normative deletion effect for events with the same `pubkey` as the deletion request publisher.
- Therefore, SKL `kind:5` revocation MUST be treated as authoritative only when the revoker pubkey equals the manifest publisher pubkey.

Example:

```json
{
  "kind": 5,
  "pubkey": "<skill_pubkey>",
  "tags": [
    ["e", "<manifest_event_id>"],
    ["a", "33400:<skill_pubkey>:<d-tag>"],
    ["k", "33400"],
    ["reason", "critical-vuln"]
  ],
  "content": "Critical security issue. Do not load."
}
```

### 5.2 Third-Party Security Warnings

Third parties (auditors, marketplaces, operators) SHOULD NOT use `kind:5` for authoritative cross-pubkey revocation. They SHOULD publish NIP-32 labels and let runtimes apply local policy.

### 5.3 Emergency Kill Practice

High-risk skills SHOULD keep a pre-signed `kind:5` for the skill publisher key in secure storage. This preserves NIP-09 semantics while enabling rapid emergency broadcast.

---

## 6. Discovery

### 6.1 Baseline Discovery (Core)

Clients discover skills by querying `kind:33400` directly.

Recommended query shape (NIP-01-compliant tag filters):

```json
["REQ", "skills", {"kinds": [33400], "#t": ["agent-skill"], "limit": 200}]
```

For a specific slug:

```json
["REQ", "skill-by-d", {"kinds": [33400], "#d": ["research-assistant"], "authors": ["<skill_pubkey_hex>"]}]
```

### 6.2 Listing-Assisted Discovery (NIP-99)

`kind:30402` listings MAY reference skill addresses via `a` or `skill` tags. SKL trust checks still run against `kind:33400`.

### 6.3 Optional NIP-90 Search Profile

Implementations MAY define a skill-search request/result pair in the NIP-90 request/result ranges (for example `5390`/`6390`).

If used, results SHOULD include `a` tags pointing to manifest addresses:

```json
["a", "33400:<skill_pubkey>:<d-tag>"]
```

SKL core does not require NIP-90 discovery support.

---

## 7. Commerce Boundary

SKL core defines identity/trust metadata only.

- License issuance and delivery are handled by NIP-SA.
- Credit/underwriting semantics are handled by NIP-AC.

If a listing is published with NIP-99 `price`, it SHOULD follow NIP-99 shape:

```json
["price", "0.00010000", "BTC"]
["price", "0.00008000", "BTC", "month"]
```

Non-standard payment rail details (Cashu mints, L402 endpoints, etc.) are profile-level extensions and MUST NOT be required for SKL core compatibility.

---

## 8. Optional Profiles (Non-Normative)

### 8.1 SKL-Bridge Profile (BreezClaw/ClaWHub Migration)

Bridge implementations MAY define import metadata such as:

- `imported_from`
- `author_handle`
- `capabilities_inferred`

Bridge metadata MUST NOT be required for SKL core validity.

If an ecosystem uses auxiliary advertisement events (including `kind:31337` conventions), that remains an ecosystem profile. SKL core discovery remains `kind:33400`-first.

### 8.2 SKL-Commerce Profile

Commercial/operator deployments MAY add policy tags for:

- gateway contracts
- payment rails
- subscription policy
- attestation quorum requirements

These profile tags MUST NOT change SKL core parsing/validation requirements.

---

## 9. SA And AC Interop

### 9.1 SA Fulfillment Link

When NIP-SA issues skill licenses/deliveries, events SHOULD reference:

- `a = 33400:<skill_pubkey>:<d-tag>`
- `e = <manifest_event_id>` (version pin)

### 9.2 AC Scope Link

When NIP-AC uses `scope=skill`, implementations SHOULD use:

```text
scope = skill:<skill_scope_id>:<constraints_hash>
```

where `skill_scope_id` is the SKL canonical format.

---

## Security Considerations

1. Manifest hash verification is mandatory before load.
2. Capability declarations are allowlists, not hints.
3. Revocation authority must respect NIP-09 pubkey semantics.
4. Delegated-signing trust elevation must validate delegation constraints/expiry if used.
5. Third-party kill signals should be labels with explicit local quorum policy.

---

## Changelog

**v3 (2026-02-26)**

- Re-scoped SKL to core registry/trust responsibilities.
- Removed BreezClaw/ClaWHub migration from normative core and moved to optional bridge profile.
- Made delegated signing optional profile rather than mandatory baseline.
- Corrected revocation semantics to align with NIP-09 same-pubkey rules.
- Removed non-standard relay filter examples (`#manifest`) in favor of NIP-01-compliant filters.
- Moved `kind:31337` usage out of core into optional ecosystem profile.
- Clarified NIP-99 `price` examples to standard shape.
- Strengthened explicit SKL links expected in SA/AC interop.

**v2**

- Expanded SKILL.md and tool schema guidance.

**v1**

- Initial draft.
