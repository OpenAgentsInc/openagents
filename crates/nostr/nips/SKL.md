# NIP-SKL

`draft` `optional`

## Agent Skill Registry

## Abstract

This NIP defines a protocol-native standard for publishing, versioning, attesting to, and revoking AI agent **Skills** as sovereign Nostr identities. Each Skill is issued its own `npub`/`nsec` keypair derived deterministically from the author's HD seed via an extended BIP-32 path building on NIP-06. A living Skill Manifest (`kind:33400`) declares capabilities, permissions, and fulfillment references. A Skill Version Log (`kind:33401`) provides an append-only, tamper-evident changelog. Attestations reuse `kind:1985` (NIP-32). Revocation reuses `kind:5` (NIP-09). Marketplace listings reuse `kind:30402` (NIP-99). Skill discovery exposes a NIP-90 DVM interface. Encrypted skill delivery and licensing fulfill via NIP-SA `kind:39220*` (Skill License) and `kind:39221*` (Skill Delivery).

**NEW in v2:** This version adds formal specification of the SKILL.md file format (§2.0), YAML-to-event derivation algorithm (§2.5), tool schema (§2.6), BreezClaw/ClaWHub compatibility mode (§1.5), subscription pricing (§6.1), and two-event discovery pattern using `kind:31337` lightweight advertisements + `kind:33400` full manifests (§7.1).

NIP-SKL is the **registry, identity, and trust layer**. NIP-SA is the **execution, delivery, and lifecycle layer**. The two NIPs are architecturally complementary, non-overlapping in kind ranges, and designed for explicit composition. Neither is complete without the other.

> ⚠️ **NOTE**: NIP-SA kind numbers (39220, 39221, and the broader 39200+ range) are not yet finalized. The OpenAgents team has noted that canonical numbers should be verified against `docs/PROTOCOL_SURFACE.md` in the OpenAgents repository before implementation. All NIP-SA kind references in this document are marked with `*` and should be treated as illustrative pending finalization.

---

## Motivation

The emergent ecosystem of AI agent Skills—reusable capability packages that instruct agents how to interact with tools, APIs, and system resources—has no protocol-native identity, versioning, or trust layer. Existing centralized registries (ClaWHub, BreezClaw marketplace) carry no cryptographic authorship, no reputation accountability, no permission declarations, and no revocation infrastructure. Empirical security research (February 2026) found that 36.82% of publicly available skills contain at least one security flaw and 13.4% contain critical-severity issues including prompt injection payloads, credential exfiltration, and persistent memory corruption.

NIP-SA's `kind:39220*`/`kind:39221*` provide the entitlement and delivery mechanism for skills, but presuppose the existence of a trust and discovery layer that is not yet specified. NIP-SKL provides that layer: agents **discover** (`kind:30402`, `kind:31337`, NIP-90 DVM), **verify** (NIP-32 attestations, NIP-26 delegation chain, capability declarations), and only then trigger the NIP-SA fulfillment flow. Without NIP-SKL, the NIP-SA skill system has no trustworthy source of truth for what a skill claims to do, who authored it, or whether it is safe to load. Without NIP-SA, NIP-SKL has no delivery or entitlement mechanism.

**This version bridges existing BreezClaw/OpenClaw ecosystems with Nostr-native trust machinery**, allowing existing skills to be imported, indexed, and gradually upgraded to full NIP-SKL compliance without breaking backward compatibility.

---

## Layered Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ HUMAN / OPERATOR                                            │
│ Configures trust tiers, approves high-risk caps            │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│ NIP-SKL (this NIP)                                          │
│ Discovery   kind:31337 lightweight ad / kind:30402 listing │
│             kind:5390 DVM query                             │
│ Identity    kind:33400 manifest (skill npub, capabilities) │
│ Trust       kind:1985 attestations (NIP-32 web-of-trust)   │
│ Versioning  kind:33401 append-only version log             │
│ Revocation  kind:5 active + pre-signed cold cert           │
└──────────────────────────┬──────────────────────────────────┘
                           │ trust checks pass
┌──────────────────────────▼──────────────────────────────────┐
│ NIP-SA (OpenAgents)                                         │
│ Entitlement  kind:39220* Skill License                     │
│ Delivery     kind:39221* Skill Delivery (NIP-44 encrypted) │
│ Execution    Agent tick cycle (perceive → think → act)     │
│ Audit        kind:39230*/39231* Trajectory events         │
└─────────────────────────────────────────────────────────────┘
```

The agent runtime MUST complete all NIP-SKL trust checks before initiating the NIP-SA `kind:39220*` license acquisition flow. The NIP-SA delivery layer MUST NOT be invoked for any skill that fails NIP-SKL trust gating.

---

## Specification

### 1. Skill Identity and Key Derivation

#### 1.1 HD Derivation Path (extends NIP-06)

NIP-06 specifies the BIP-32 path:

```
m/44'/1237'/account'/0/0
```

NIP-SA extends this for agent accounts. NIP-SKL extends it further for skill keypairs:

```
m/44'/1237'/agent_account'/skill_type'/skill_index'
```

| Segment | Value | Description |
|---------|-------|-------------|
| `44'` | 44 (hardened) | BIP-44 purpose |
| `1237'` | 1237 (hard.) | SLIP-44 coin type for Nostr |
| `agent_account'` | 0, 1, 2... | NIP-06 / NIP-SA agent account index |
| `skill_type'` | 0–255 (hard.) | Capability category (§1.2) |
| `skill_index'` | 0, 1, 2... | Sequential skill index within type |

All segments MUST use hardened derivation (`'`). The `agent_account'` index is shared with the owning NIP-SA agent, making the parent-child relationship auditable from a single BIP-39 mnemonic.

**Example key tree:**

```
BIP-39 mnemonic (cold storage)
  └── m/44'/1237'/0'/0/0     NIP-SA Agent npub (hot)
      └── m/44'/1237'/0'/1'/0'   Skill: payment type, index 0
      └── m/44'/1237'/0'/8'/0'   Skill: eCash type, index 0
      └── m/44'/1237'/0'/3'/0'   Skill: shell type, index 0
```

#### 1.2 Skill Type Values

| `skill_type'` | Category |
|---------------|----------|
| `0'` | General purpose (no elevated permissions) |
| `1'` | Financial / payment-adjacent |
| `2'` | Communication / messaging |
| `3'` | System / shell access |
| `4'` | File system read/write |
| `5'` | External API / HTTP outbound |
| `6'` | Memory / agent state modification |
| `7'` | Identity / credential management |
| `8'` | eCash / bearer token operations (Cashu) |
| `9'` | Federation / multi-party custody (Fedimint) |
| `10–127'` | Reserved for future standard types |
| `128–255'` | Application-defined types |

#### 1.3 NIP-05 Identifier

Skills SHOULD publish a NIP-05 identifier:

```
<skill-slug>@skills.<author-domain>
```

Example: `satnam-pay@skills.satnam.pub`

#### 1.4 Delegation Chain (NIP-26)

```
author root npub (cold)
  └── NIP-SA agent npub (NIP-26 delegated, kind:39200* scope)
      └── skill npub (NIP-26 delegated, kind:33400+33401 scope only)
```

The skill's NIP-26 delegation MUST be scoped to `kind:33400` and `kind:33401` only, with an expiry matching the manifest's `expiry` tag.

Agent runtimes MUST verify the full two-hop delegation chain: `skill_npub → agent_npub → author root npub`. A one-hop chain (skill directly to author, bypassing the agent) is valid only for author-operated single-agent deployments and MUST be explicitly flagged in the manifest with `["single_hop", "true"]`.

#### 1.4.1 Delegation Verification Algorithm (normative)

Given a candidate manifest event (`kind:33400`) signed by `skill_npub`, runtimes MUST:

1. Verify the manifest signature against `skill_npub`.
2. Resolve a valid NIP-26 delegation from `agent_npub` authorizing `skill_npub` for `kind:33400` and `kind:33401` only, with non-expired delegation window.
3. Resolve a valid NIP-26 delegation from `author_npub` authorizing `agent_npub` for NIP-SA agent kinds.
4. Verify both delegations are non-expired at manifest `created_at` and at current evaluation time.
5. Reject the manifest if any hop is missing, expired, kind-overbroad, or signature-invalid.

Implementations SHOULD cache positive delegation-chain checks by `(skill_npub, agent_npub, author_npub, expiry)` for efficient replay-safe validation.

#### 1.5 Compatibility Mode (BreezClaw/ClaWHub Migration)

**Purpose**: Allow third-party importers (like Satnam NAVIGATOR) to create NIP-SKL manifests for existing BreezClaw skills without original author participation.

**Rules for imported skills:**

1. **Trust tier**: MUST default to `none` (unattested)
2. **Required tag**: `["imported_from", "<github_url_or_clawhub_url>"]`
3. **Author field**: Use GitHub username → npub mapping service OR leave as `["author_handle", "<github_username>"]` if no npub available
4. **Skill keypair**: Importer MAY generate ephemeral keypair OR leave `skill_npub` as placeholder `npub1importerplaceholder...` with note that full NIP-SKL adoption requires author re-publication
5. **Capability inference**: Parse SKILL.md content to infer likely capabilities; mark with `["capabilities_inferred", "true"]`
6. **Attestation path**: Community members MAY attest to imported skills, but trust tier cannot exceed `marginal` until author claims ownership via `["author_claim", "<signature_proof>"]` event

**Author claim process:**

To claim an imported skill, original author publishes:

```json
{
  "kind": 1985,
  "pubkey": "<verified_author_npub>",
  "tags": [
    ["L", "skill-ownership"],
    ["l", "author-claim", "skill-ownership"],
    ["e", "<imported_kind:33400_event_id>"],
    ["p", "<importer_npub>"],
    ["proof", "<signature_of_github_commit_or_repo_ownership>"]
  ],
  "content": "I am the original author of this skill. Claiming ownership.",
  "sig": "<signature>"
}
```

After verified claim, importer or author SHOULD publish updated `kind:33400` with proper delegation chain.

---

### 2. Skill Manifest (kind:33400)

Parameterized replaceable event (NIP-01). Relays store only the most recent version per `(pubkey, kind, d-tag)` tuple. Full version history is preserved in `kind:33401`.

#### 2.0 SKILL.md File Format Specification (NEW)

**The SKILL.md file is the hash-committed payload for NIP-SA delivery.** It consists of:

1. **YAML frontmatter** (delimited by `---`)
2. **Markdown body** (description, installation, examples, tool documentation)

**Minimal BreezClaw-compatible SKILL.md:**

```markdown
---
slug: example-skill
name: Example Skill
description: "Single-sentence description of what this skill does and when to use it"
version: 1.0.0
author: github-username
keywords: [keyword1, keyword2, keyword3]
homepage: https://github.com/author/repo
---

# Example Skill

Longer description here...

## Install

Installation instructions...

## Tools

| Tool | Description |
|------|-------------|
| tool_name | What it does |
```

**Extended NIP-SKL SKILL.md (adds trust/payment fields):**

```markdown
---
slug: example-skill
name: Example Skill
description: "Single-sentence description"
version: 1.0.0
author: github-username
author_npub: npub1abc...
keywords: [keyword1, keyword2, keyword3]
homepage: https://github.com/author/repo

# NIP-SKL Extensions
agent_identity:
  nip05: example-skill@skills.domain.com
  nostr_pubkey: npub1def...  # skill's own npub
  lightning_address: example-skill@domain.com
  cashu_mint: https://mint.domain.com

pricing:
  model: per_call  # or: free, subscription
  base_sats: 100
  currency: BTC
  accept_cashu: true
  subscription_period: monthly  # if model: subscription

gateway:
  url: https://gateway.domain.com/v1/skills/example-skill
  auth: L402  # or: none
  macaroon_endpoint: https://gateway.domain.com/v1/auth/macaroon
  timeout_seconds: 30

requires:
  - SKILL_API_KEY
  - SKILL_ENDPOINT
optional:
  - SKILL_DEBUG

capabilities:
  - http:outbound
  - filesystem:read

tools:
  - name: example_tool
    description: "What this tool does"
    parameters:
      - name: param1
        type: string
        required: true
        description: "Parameter description"
    returns:
      type: object
      description: "Return value description"
---

# Example Skill

Full markdown documentation...
```

**Compliance levels:**

- **BreezClaw-only fields** → `none` tier NIP-SKL compatible (discoverable, not trusted)
- **+ `author_npub` + `capabilities`** → Can reach `marginal` tier with community attestations
- **+ `agent_identity` + `pricing` + `gateway`** → Full NIP-SKL compatible, can reach `full`/`ultimate` tiers

**Validation rules:**

1. `slug` MUST be unique per author, kebab-case, alphanumeric + hyphens only
2. `version` MUST be valid semver (MAJOR.MINOR.PATCH)
3. `description` MUST be ≤280 characters (single sentence)
4. `keywords` MUST be array of lowercase strings, no commas within strings
5. `capabilities` array maps 1:1 to capability flags in §2.2
6. `tools` array follows JSON Schema in §2.6

#### 2.1 Full Event Structure (kind:33400)

```json
{
  "kind": 33400,
  "pubkey": "<skill_npub>",
  "created_at": <unix_timestamp>,
  "tags": [
    ["d", "<slug-from-frontmatter>"],
    ["name", "<name from frontmatter>"],
    ["version", "<semver from frontmatter>"],
    ["description", "<description from frontmatter>"],
    ["author_npub", "<root author npub>"],
    ["author_handle", "<github_username>"],  // optional
    ["agent_npub", "<NIP-SA agent npub that owns this skill>"],
    ["skill_type", "<0-255>"],
    ["skill_index", "<0-N>"],
    ["capability", "<capability_flag>"],  // repeated per capability
    ["expiry", "<unix_timestamp>"],
    ["manifest_hash", "<sha256 of SKILL.md content>"],
    ["skill_file", "<blossom URL or NIP-94 event id of SKILL.md>"],
    ["skill_scope_id", "33400:<skill_npub>:<d-tag>:<version>"],
    ["license_kind", "39220", "<marketplace_npub>", "<relay_url>"],
    ["delivery_kind", "39221", "<skill_provider_npub>", "<relay_url>"],
    ["trajectory_session_kind", "39230", "<relay_url>"],
    ["trajectory_event_kind", "39231", "<relay_url>"],
    ["pre_revocation_cert", "<blossom URL or NIP-94 event id>"],
    ["l402_endpoint", "<macaroon_acquisition_url>"],  // NEW
    ["v", "<previous kind:33400 event id>"],
    ["p", "<author_npub>", "<relay_url>"],
    ["zap", "<author_npub>", "<relay_url>", "1"],
    ["t", "agent-skill"],
    ["t", "<keyword>"]  // repeated per keyword
  ],
  "content": "<brief changelog for this version>",
  "sig": "<schnorr signature by skill_npub>"
}
```

**NEW tags:**

- `["author_handle", "<github_username>"]` — Human-readable author identifier for BreezClaw migration
- `["l402_endpoint", "<url>"]` — Direct L402 macaroon acquisition endpoint
- `["skill_scope_id", "33400:<skill_npub>:<d-tag>:<version>"]` — Canonical skill scope id for NIP-AC interop
- `["imported_from", "<url>"]` — Only for compatibility-mode imports (§1.5)
- `["capabilities_inferred", "true"]` — Flags auto-inferred capabilities needing author verification

#### 2.2 Capability Flags

`capability` MUST be declared once per required permission. Agent runtimes MUST NOT grant undeclared permissions. Any undeclared capability exercised at runtime MUST trigger immediate suspension and a `kind:1985` label of `capability-violation`.

**System Capabilities:**

| Flag | Meaning | Min Trust Tier |
|------|---------|----------------|
| `none` | No system permissions required | `none` |
| `filesystem:read` | Read local files | `marginal` |
| `filesystem:write` | Write or delete local files | `marginal` |
| `shell:exec` | Execute shell commands | `full` + pre-rev cert |
| `http:outbound` | Make outbound HTTP/S requests | `none` |
| `http:domains:<list>` | Outbound HTTP to specific allowlisted domains | `none` |
| `memory:read` | Read agent memory / SOUL.md | `marginal` |
| `memory:write` | Write or modify agent memory | `full` |
| `credentials:read` | Access stored credentials or API keys | `full` |
| `nostr:publish` | Publish Nostr events on behalf of agent | `marginal` |
| `nostr:dm` | Send Nostr DMs on behalf of agent | `marginal` |

**Lightning / Bitcoin:**

| Flag | Meaning | Min Trust Tier |
|------|---------|----------------|
| `payment:lightning` | Create and pay Lightning invoices (send+recv) | `full` |
| `payment:lightning:send` | Pay invoices only | `full` |
| `payment:lightning:recv` | Create invoices only | `marginal` |
| `payment:onchain` | Initiate on-chain Bitcoin transactions | `ultimate` + approval |
| `payment:l402` | Operate as or consume an L402-gated service | `marginal` |

**Cashu eCash:**

| Flag | Meaning | Min Trust Tier |
|------|---------|----------------|
| `payment:cashu` | General Cashu eCash operations | `full` |
| `payment:cashu:mint` | Mint tokens from a Lightning payment | `full` |
| `payment:cashu:melt` | Melt tokens back to Lightning | `full` |
| `payment:cashu:send` | Transfer tokens to another party | `full` |
| `payment:cashu:recv` | Receive tokens from another party | `marginal` |
| `payment:cashu:bond` | Lock tokens as a performance bond | `full` |
| `payment:cashu:bond:slash` | Slash (burn) a locked performance bond | `ultimate` |
| `payment:cashu:multimint` | Interact with more than one Cashu mint | `full` |

Skills with `payment:cashu:bond:slash` MUST have `ultimate`-tier attestation and MUST declare a `bond_arbiter` tag:

```json
["bond_arbiter", "<arbiter_npub>", "<relay_url>"]
```

**Fedimint:**

| Flag | Meaning | Min Trust Tier |
|------|---------|----------------|
| `payment:fedimint` | General Fedimint federation interaction | `full` |
| `payment:fedimint:deposit` | Deposit Bitcoin into a federation | `full` |
| `payment:fedimint:withdraw` | Withdraw from a federation to Lightning | `full` |
| `payment:fedimint:ecash` | Transfer eCash within a federation | `full` |
| `payment:fedimint:gateway` | Operate as or use a Fedimint Lightning gateway | `full` |
| `payment:fedimint:multifed` | Interact with more than one federation | `full` |
| `payment:fedimint:admin` | Administrative / guardian federation ops | `ultimate` + approval |

Skills with any `payment:fedimint:*` capability MUST declare target federation(s):

```json
["federation", "<federationId>", "<invite_code_or_meta_url>"]
```

Skills with any `payment:cashu:*` capability MUST declare target mint(s):

```json
["mint", "<mint_url>", "<nut_list>"]
```

#### 2.3 Delivery Hash Verification

The `manifest_hash` field is the SHA-256 of the canonical SKILL.md payload bytes that will be delivered via NIP-SA `kind:39221*`. Canonical bytes are:

1. UTF-8 encoding
2. no BOM
3. LF (`\n`) line endings
4. exact payload bytes post-NIP-44 decrypt

Agent runtimes MUST verify `sha256(canonical_decrypted_payload) == manifest_hash` before loading. Hash mismatch MUST result in skill rejection and a `kind:1985 delivery-hash-mismatch` attestation.

#### 2.4 Expiry and Renewal

**Maximum recommended expiry windows:**

- General skills: 180 days
- Skills with any `payment:*` capability: 90 days
- Skills with `payment:onchain` or `payment:fedimint:admin`: 30 days

Renewal requires: new `kind:33400` with updated `expiry`, extended NIP-26 delegation, fresh `manifest_hash`, and new NIP-SA `license_kind` / `delivery_kind` references if relay locations changed.

#### 2.4.1 Canonical Skill Scope ID (NIP-AC Interop)

For NIP-AC `scope=skill` compatibility, SKL defines:

```text
skill_scope_id = "33400:<skill_npub>:<d-tag>:<version>"
```

When NIP-AC envelopes target a skill invocation, implementations SHOULD use:

```text
scope = "skill:<skill_scope_id>:<constraints_hash>"
```

#### 2.5 YAML-to-kind:33400 Derivation Algorithm (NEW)

**Purpose**: deterministic derivation of manifest payload/tags from SKILL.md frontmatter, reducing manual divergence.

**Determinism scope**:

- Deterministic: `kind`, `pubkey`, `tags`, `content`.
- Caller-supplied (non-deterministic unless fixed by caller): `created_at`.
- Signature depends on the finalized event and signer key.

**Algorithm:**

```
INPUT:
  - SKILL.md file with YAML frontmatter
  - required skill_npub
  - required created_at (unix timestamp)
OUTPUT:
  - unsigned kind:33400 event

1. Parse YAML frontmatter into object `fm`.

2. Validate required fields:
   slug, name, version, description, author_npub, capabilities (or empty list).
   Fail closed on validation errors.

3. Canonicalize SKILL.md payload bytes:
   - UTF-8, no BOM, LF line endings.
   - hash = sha256(canonical_payload_bytes).

4. Initialize event:
   event = {
     kind: 33400,
     pubkey: skill_npub,
     created_at: created_at,
     tags: [],
     content: "",
     sig: null
   }

5. Map required tags:
   ["d", fm.slug]
   ["name", fm.name]
   ["version", fm.version]
   ["description", fm.description]
   ["author_npub", fm.author_npub]
   ["manifest_hash", hash]

6. Map identity/ownership tags when available:
   ["author_handle", fm.author] (optional)
   ["agent_npub", configured_agent_npub] (if configured)
   ["skill_scope_id", "33400:<skill_npub>:<d-tag>:<version>"]

7. Map capabilities:
   - emit one ["capability", value] per capability.
   - emit ["capability", "none"] only when no capabilities declared.

8. Map gateway/requirements/tools:
   - ["l402_endpoint", ...] when L402 is configured.
   - ["env_required", ...] per required env var.
   - ["env_optional", ...] per optional env var.
   - ["tool", tool_name, canonical_json(tool_schema)] per tool.

9. Map indexing and expiry:
   - ["t", "agent-skill"]
   - ["t", keyword] for each keyword.
   - ["expiry", explicit_or_default_expiry].

10. Map SA fulfillment references (if configured):
    ["license_kind", "39220", "<marketplace_npub>", "<relay_url>"]
    ["delivery_kind", "39221", "<skill_provider_npub>", "<relay_url>"]
    ["trajectory_session_kind", "39230", "<relay_url>"]
    ["trajectory_event_kind", "39231", "<relay_url>"]

11. Canonicalize tag order:
    sort by tag key, then full tuple value, preserving duplicate-key entries.

12. Set content:
    deterministic changelog summary or explicit caller-provided summary.

13. Return unsigned event for signing by skill_npub.
```

**Reference implementation:** See `scripts/yaml-to-kind33400.js` in reference implementation repo.

#### 2.6 Tool Schema (NEW)

**Purpose**: Standardize tool definitions in SKILL.md for machine parsing, mapping to MCP, OpenClaw, and NIP-90 DVM interfaces.

**YAML schema in frontmatter:**

```yaml
tools:
  - name: tool_name
    description: "What this tool does and when to use it"
    parameters:
      - name: param1
        type: string  # string | number | boolean | object | array
        required: true
        description: "Parameter description"
        default: "default_value"  # optional
        enum: [option1, option2]  # optional
      - name: param2
        type: number
        required: false
        description: "Optional parameter"
    returns:
      type: object
      description: "Description of return value"
      properties:  # optional, for structured returns
        field1:
          type: string
          description: "Field description"
```

**Mapping to MCP tool definition:**

```json
{
  "name": "tool_name",
  "description": "What this tool does and when to use it",
  "inputSchema": {
    "type": "object",
    "properties": {
      "param1": {
        "type": "string",
        "description": "Parameter description"
      },
      "param2": {
        "type": "number",
        "description": "Optional parameter"
      }
    },
    "required": ["param1"]
  }
}
```

**Mapping to OpenClaw `registerTool()` signature:**

```typescript
registerTool({
  name: 'tool_name',
  description: 'What this tool does and when to use it',
  parameters: z.object({
    param1: z.string().describe('Parameter description'),
    param2: z.number().optional().describe('Optional parameter')
  })
})
```

**Mapping to NIP-90 DVM job params:**

```json
{
  "kind": 5390,
  "tags": [
    ["i", "<user_input>", "text"],
    ["param", "param1", "<value>"],
    ["param", "param2", "<value>"]
  ]
}
```

**Storage in kind:33400:**

Tools are stored as JSON-encoded tag arrays:

```json
["tool", "tool_name", "{\"name\":\"tool_name\",\"description\":\"...\",\"parameters\":[...]}"]
```

This allows relay indexing by tool name and full schema retrieval without fetching the SKILL.md file.

---

### 3. Skill Version Log (kind:33401)

Regular (non-replaceable) events forming an append-only changelog.

```json
{
  "kind": 33401,
  "pubkey": "<skill_npub>",
  "created_at": <unix_timestamp>,
  "tags": [
    ["d", "<same skill identifier as kind:33400>"],
    ["version", "<semver>"],
    ["prev_version", "<previous semver>"],
    ["manifest_event", "<kind:33400 event id this entry references>"],
    ["manifest_hash", "<sha256 of SKILL.md at this version>"],
    ["license_kind", "39220"],
    ["delivery_kind", "39221"],
    ["change_type", "<added|changed|fixed|deprecated|security>"]
  ],
  "content": "<human-readable changelog entry>",
  "sig": "<schnorr signature by skill_npub>"
}
```

Relays SHOULD retain all `kind:33401` events indefinitely. Clients MAY use this log to detect version rollback attacks.

---

### 4. Attestation and Trust (kind:1985, NIP-32)

#### 4.1 Attestation Event

```json
{
  "kind": 1985,
  "pubkey": "<attester_npub>",
  "tags": [
    ["L", "skill-security"],
    ["l", "<attestation_label>", "skill-security"],
    ["p", "<skill_npub>"],
    ["e", "<kind:33400 manifest event id>"],
    ["version", "<semver attested>"],
    ["tool", "<scanner name and version, if automated>"]
  ],
  "content": "<optional human-readable notes>",
  "sig": "<schnorr signature by attester_npub>"
}
```

#### 4.2 Attestation Labels

| Label | Meaning | Effect on Trust |
|-------|---------|-----------------|
| `audit-passed` | Full human review, no issues found | Enables `full` tier |
| `scan-clean` | Automated scanner clear at stated version | Enables `marginal` tier |
| `community-vouched` | Community member vouch (zap-weighted) | Accumulates toward `marginal` |
| `capabilities-verified` | Declared capabilities match actual behavior | Required for `full` |
| `payment-flows-verified` | Payment capability flows tested vs declared mint/federation | Required for payment caps |
| `delivery-hash-verified` | NIP-44 delivery payload matches manifest_hash | Runtime verification |
| `bond-active` | Cashu performance bond currently locked | Economic accountability |
| `malicious-confirmed` | Confirmed malicious payload — **KILL FLAG** | Forces `none` only when kill-authority quorum passes |
| `prompt-injection` | Prompt injection detected — **KILL FLAG** | Forces `none` only when kill-authority quorum passes |
| `credential-exfil` | Credential exfiltration detected — **KILL FLAG** | Forces `none` only when kill-authority quorum passes |
| `capability-violation` | Exercised undeclared capability at runtime — **KILL FLAG** | Forces `none` only when kill-authority quorum passes |
| `delivery-hash-mismatch` | NIP-44 payload does not match manifest_hash — **KILL FLAG** | Forces `none` only when kill-authority quorum passes |
| `bond-slashed` | Performance bond burned after kill flag — **KILL FLAG** | Permanent reputation damage |
| `abandoned` | Author unresponsive, skill unmaintained | Warning only |
| `superseded` | Replaced; include `["e", "<new_skill_event_id>"]` | Redirect to new version |

#### 4.3 Trust Tiers

| Tier | GPG Equivalent | Assignment |
|------|----------------|------------|
| `ultimate` | ultimate | Keys in operator's own NIP-26 delegation tree |
| `full` | full | ≥1 `audit-passed` + `capabilities-verified` attestations from configured trusted attester npubs |
| `marginal` | marginal | ≥1 `scan-clean` or ≥3 `community-vouched` with combined zap weight ≥1000 sats |
| `none` | untrusted | Default for all unattested skills |

#### 4.4 Zap-Weighted Trust

NIP-57 zap receipts co-published with `kind:1985` marginal attestations carry weight proportional to their millisatoshi amount. The threshold for promoting a `marginal` attestation to functional `full` weight is intentionally left implementation-defined. Implementations MUST surface this threshold to operators for explicit configuration.

**Recommended default:** 3 `community-vouched` attestations with combined zap weight ≥1000 sats = `marginal` tier.

#### 4.5 Kill-Flag Authority and Quorum (normative)

Kill-flag labels MUST NOT be treated as authoritative solely because they exist on relays.

A runtime MUST apply a kill flag only if one of the following is true:

1. label is issued by an `ultimate`-tier key in the operator trust root, or
2. label is issued by a configured `full` security attester and corroborated by at least one additional configured `full` or two configured `marginal` attesters for the same `(skill_npub, version, label)`.

Until quorum is met, runtimes SHOULD mark the skill as `under_review` rather than hard-blocking execution.

---

### 5. Revocation

#### 5.1 Active Revocation (NIP-09)

```json
{
  "kind": 5,
  "pubkey": "<author_npub OR attester_npub>",
  "tags": [
    ["e", "<kind:33400 manifest event id>"],
    ["a", "33400:<skill_npub>:<d-tag>"],
    ["e", "<kind:39220* license event id, if known>"],
    ["reason", "<human-readable reason>"]
  ],
  "content": "<revocation reason>",
  "sig": "<schnorr signature>"
}
```

The `["e", "<kind:39220* license event id>"]` tag signals to NIP-SA runtimes that the corresponding license should also be treated as revoked. NIP-SA implementations SHOULD respect this signal and refuse delivery for licenses referenced in a valid revocation event.

#### 5.2 Pre-Generated Revocation Certificate

Required for all skills with any `payment:*`, `shell:exec`, or `memory:write` capability:

```json
["pre_revocation_cert", "<blossom URL or NIP-94 event id>"]
```

The pre-signed `kind:5` MUST:

1. Be signed by the `ultimate`-tier root `npub`
2. Reference the manifest identity via `["a", "33400:<skill_npub>:<d-tag>"]`
3. MAY include specific `kind:39220*` license event ids when already known, but MUST NOT require unknown future license ids
4. Be encrypted (NIP-44) to the author's cold storage key
5. Be stored out-of-band (e.g., Vaultwarden on Start9)

---

### 6. Marketplace Listing (kind:30402, NIP-99)

#### 6.0 Basic Listing

```json
{
  "kind": 30402,
  "pubkey": "<author_npub>",
  "tags": [
    ["d", "<skill-identifier>"],
    ["title", "<skill name>"],
    ["summary", "<one-line description>"],
    ["skill", "33400:<skill_npub>:<d-tag>"],
    ["t", "agent-skill"],
    ["t", "<keyword>"],
    ["published_at", "<unix_timestamp>"],
    ["price", "<amount>", "msats"],
    ["price", "<amount>", "cashu", "<mint_url>"]
  ],
  "content": "<full markdown description>",
  "sig": "<sig by author_npub>"
}
```

#### 6.1 Subscription Pricing Tag (NEW)

**Purpose**: Support recurring billing for agent skills (subscriptions are a distinct commercial pattern from one-time purchases).

**Tag format:**

```json
["price", "<amount>", "msats", "monthly"]
["price", "<amount>", "msats", "yearly"]
["price", "<amount>", "cashu", "<mint_url>", "monthly"]
```

**Subscription periods:**

- `monthly` — 30-day billing cycle
- `quarterly` — 90-day billing cycle
- `yearly` — 365-day billing cycle
- `weekly` — 7-day billing cycle (discouraged for agent skills)

**Example: Family office skill with monthly subscription:**

```json
{
  "kind": 30402,
  "pubkey": "<author_npub>",
  "tags": [
    ["d", "satnam-family"],
    ["title", "Satnam Family Office Manager"],
    ["summary", "Sovereign family identity and delegation management"],
    ["skill", "33400:<skill_npub>:satnam-family"],
    ["t", "agent-skill"],
    ["t", "family-office"],
    ["published_at", "1740000000"],
    ["price", "2100000", "msats", "monthly"],
    ["price", "21000000", "msats", "yearly"],
    ["price", "2100", "cashu", "https://mint.satnam.pub", "monthly"]
  ],
  "content": "Full markdown description...",
  "sig": "<signature>"
}
```

**Fulfillment semantics:**

- NIP-SA `kind:39220*` license events for subscriptions SHOULD include `["subscription_period", "monthly"]` and `["expires_at", "<timestamp>"]` tags
- Agents SHOULD auto-renew subscriptions 24 hours before expiry if the skill is still loaded
- Subscription payment flows via Lightning SHOULD use NIP-57 zap with `["subscription", "renewal"]` tag
- Failed subscription renewals SHOULD trigger skill unload and `kind:1985` attestation with label `subscription-expired`

**Backward compatibility:** Listings without subscription period in price tag are interpreted as one-time purchase (existing behavior).

---

### 7. Skill Discovery

#### 7.0 Two-Event Discovery Pattern (NEW)

**Purpose**: Bridge lightweight discovery (kind:31337) used by OpenClaw/ClaWHub with full NIP-SKL trust machinery (kind:33400).

**Pattern: Skills publish BOTH events**

1. **kind:31337** — Lightweight advertisement for fast discovery
2. **kind:33400** — Full manifest with capabilities, delegation, attestations

**kind:31337 Lightweight Advertisement:**

```json
{
  "kind": 31337,
  "pubkey": "<skill_npub>",
  "created_at": <unix_timestamp>,
  "tags": [
    ["d", "<skill-slug>"],
    ["name", "<skill name>"],
    ["description", "<single sentence>"],
    ["t", "agent-skill"],
    ["t", "<keyword>"],
    ["manifest", "33400:<skill_npub>:<d-tag>"],  // Points to full manifest
    ["price", "<amount>", "msats"],  // Optional quick price signal
    ["nip05", "<skill-slug>@skills.<domain>"],
    ["lud16", "<skill-slug>@<domain>"]
  ],
  "content": "",
  "sig": "<signature>"
}
```

**Discovery flow:**

```
1. Agent queries kind:31337 with ["t", "agent-skill"] → fast index scan
2. Agent finds candidate skill(s)
3. Agent fetches referenced kind:33400 via ["manifest", "..."] tag
4. Agent performs NIP-SKL trust gating on kind:33400
5. If trust checks pass → proceed to NIP-SA fulfillment
```

**Why two events?**

- **kind:31337** is optimized for speed: small event size, no delegation verification, used by ClaWHub/OpenClaw
- **kind:33400** is optimized for trust: full capability declarations, attestation references, NIP-26 chain verification
- Existing BreezClaw skills can publish kind:31337 immediately for discovery while gradually adopting kind:33400 for trust
- Agents can choose discovery strategy: fast (31337-only) vs trustworthy (31337→33400 chain)

**Relay behavior:**

- Relays supporting NIP-SKL SHOULD index both kinds with cross-references
- Relays MAY provide filtered queries: `["kinds":[31337],"#manifest":["33400:<npub>:<d>"]]` returns only skills with full manifests

**Compatibility:**

- Skills without kind:33400 remain discoverable via kind:31337 but receive `none` trust tier
- Skills with kind:33400 but no kind:31337 are discoverable via kind:30402 or NIP-90 DVM (§7.1)

#### 7.1 NIP-90 DVM Discovery

**Job Request** (`kind:5390`)

```json
{
  "kind": 5390,
  "pubkey": "<requesting_agent_npub>",
  "tags": [
    ["i", "<search query or capability filter>", "text"],
    ["param", "skill_type", "<0-255>"],
    ["param", "capability", "<capability_flag>"],
    ["param", "min_trust", "<none|marginal|full|ultimate>"],
    ["param", "exclude_stale","true"],
    ["param", "exclude_kill_flagged", "true"],
    ["relays", "<relay_url>", "<relay_url>"]
  ],
  "content": ""
}
```

**Job Result** (`kind:6390`)

```json
{
  "kind": 6390,
  "pubkey": "<dvm_npub>",
  "tags": [
    ["request", "<kind:5390 event id>"],
    ["e", "<kind:33400 manifest event id>", "", "skill"],
    ["e", "<kind:31337 ad event id>", "", "advertisement"],  // NEW
    ["p", "<requesting_agent_npub>"]
  ],
  "content": "<JSON array of matching kind:33400 event ids>"
}
```

DVM operators indexing NIP-SKL SHOULD also index:
- Corresponding NIP-SA `kind:39220*` licenses
- kind:31337 advertisements
- kind:30402 marketplace listings

This allows single DVM query to return complete discovery context.

---

### 8. Relay Recommendations

Relays supporting this NIP:

- SHOULD store `kind:33400`, `kind:33401`, `kind:31337`, `kind:1985` tagged `["t", "agent-skill"]`
- SHOULD store `kind:5390` and `kind:6390` DVM events
- SHOULD enforce `kind:33401` immutability
- SHOULD propagate `kind:5` revocations for `kind:33400` events
- SHOULD honor `expiry` tags
- SHOULD co-index NIP-SA `kind:39220*` events referencing a known `skill_manifest` tag
- SHOULD index kind:31337 `["manifest", "..."]` tags for fast 31337→33400 lookups
- MAY expose a dedicated skills discovery endpoint
- MAY implement trust-tier filtering for queries

---

### 9. Complete Lifecycle

```
GENESIS
  BIP-39 mnemonic (shared with NIP-SA agent)
  BIP-32 → m/44'/1237'/0'/8'/0' (eCash type, skill #0)
  NIP-26 delegation: author root → agent npub → skill npub
  Pre-signed NIP-44-encrypted kind:5 → cold storage

AUTHOR
  Write SKILL.md with YAML frontmatter
  Run yaml-to-kind33400.js derivation script
  Sign resulting kind:33400 with skill_npub
  Publish kind:33400 (full manifest)
  Publish kind:31337 (lightweight ad) pointing to kind:33400
  Publish kind:33401 (initial version log)
  Publish kind:30402 (marketplace listing)

ATTEST
  Auditors publish kind:1985 (audit-passed, capabilities-verified)
  Automated scanner publishes kind:1985 (scan-clean)
  Community zaps skill_npub + kind:1985 (community-vouched)
  If payment capability: Cashu bond locked; kind:1985 bond-active published

DISCOVER
  Agent queries kind:31337 OR kind:30402 OR kind:5390 DVM
  Agent fetches kind:33400 via ["manifest", ...] reference
  Agent performs trust gate (§2.1)

ACQUIRE (NIP-SA fulfillment, after NIP-SKL trust gate passes)
  Agent requests license acquisition for kind:33400
  Marketplace issues kind:39220* Skill License
  Skill provider (or delegated delivery service) sends kind:39221* NIP-44 encrypted payload to agent npub
  Agent verifies: sha256(decrypted payload) == manifest_hash
  If mismatch: reject + publish kind:1985 delivery-hash-mismatch

EXECUTE
  Skill loaded within declared capability sandbox
  Any undeclared capability → suspend + kind:1985 capability-violation
  Agent publishes kind:39230*/39231* trajectory events (NIP-SA audit log)

RENEW (before expiry)
  New kind:33400 with updated expiry + manifest_hash
  New kind:33401 version log entry with manifest_event + prev_version chain
  Extended NIP-26 delegation
  Update kind:31337 to point to new kind:33400

REVOKE
  Publish kind:5 referencing both kind:33400 AND kind:39220* license
  OR retrieve and broadcast pre-signed cold kind:5
  If bond active: bond-slashed kind:1985 published
  NIP-SA runtimes: refuse delivery for revoked license event ids
  All runtimes: unload skill immediately
```

---

## Event Kinds Summary

### Introduced by NIP-SKL

| Kind | Type | Description |
|------|------|-------------|
| 33400 | Parameterized repl. | Skill Manifest (full trust-gated) |
| 33401 | Regular (append-only) | Skill Version Log |
| 31337 | Parameterized repl. | Lightweight Advertisement (fast discovery) |
| 5390 | Regular | NIP-90 Skill Search Job Request |
| 6390 | Regular | NIP-90 Skill Search Job Result |

### Reused from Other NIPs

| Kind | NIP | Usage |
|------|-----|-------|
| 1985 | NIP-32 | Attestation labels and kill flags |
| 30402 | NIP-99 | Marketplace storefront listing |
| 5 | NIP-09 | Active and pre-generated revocation |
| 39220* | NIP-SA | Skill License (fulfillment entitlement) |
| 39221* | NIP-SA | Skill Delivery (NIP-44 encrypted content) |
| 39230*/39231* | NIP-SA | Trajectory session/events (execution audit log) |

---

## Implementation Priorities

**Phase 1: Core Identity (Weeks 1-2)**
- BIP-32 skill key derivation
- kind:33400 manifest publishing
- YAML-to-kind:33400 derivation script
- SHA-256 manifest hash verification

**Phase 2: Discovery (Weeks 3-4)**
- kind:31337 lightweight advertisement
- kind:30402 marketplace listings
- Two-event discovery pattern (31337→33400)
- Basic relay indexing

**Phase 3: Trust Layer (Weeks 5-6)**
- kind:1985 attestation framework
- Trust tier calculation
- Capability enforcement in agent runtime
- NIP-26 delegation chain verification

**Phase 4: BreezClaw Bridge (Week 7)**
- Compatibility mode importer (§1.5)
- GitHub→npub mapping service
- ClaWHub skill indexer
- Author claim process

**Phase 5: Payment Integration (Weeks 8-9)**
- Subscription pricing tags (§6.1)
- L402 endpoint integration
- Cashu bond mechanics
- NIP-SA fulfillment flow

**Phase 6: Production Hardening (Week 10+)**
- Pre-signed revocation certificates
- Relay optimization
- DVM indexer deployment
- Security audit

---

## Reference Implementations

- **satnam_pub** — Full NIP-SKL implementation with PhoenixD/LNbits/Cashu/Headscale gateway
  - Repo: `https://github.com/OV1-Kenobi/satnam_pub`
  - Skills: NAVIGATOR, SYBIL, HERMES, MINT, CASHU, FROST, DELEGATR, ATTEST, STEWARD, AUDIT

- **openclaw-skill-bridge** — BreezClaw→NIP-SKL compatibility importer
  - Repo: TBD
  - Function: Scan ClaWHub, generate kind:33400 manifests for existing skills

- **yaml-to-kind33400** — Derivation script (§2.5)
  - Repo: `satnam_pub/scripts/yaml-to-kind33400.js`
  - Usage: `node yaml-to-kind33400.js skills/satnam-pay/SKILL.md`

---

## Security Considerations

1. **Key Compromise**: Skill hot keys are expendable; BIP-32 hardened derivation prevents sibling key exposure. Pre-signed revocation certs enable cold-stored kill switch.

2. **Capability Sandbox Escape**: Agent runtimes MUST enforce capability restrictions at syscall/API level, not just declaration level. Violations trigger immediate `capability-violation` attestation.

3. **Supply Chain Attacks**: Trust tier minimums + mandatory attestations + performance bonds create economic deterrent. No skill with `payment:*` capability can load without `full` tier attestation.

4. **Replay Attacks**: kind:33400 `expiry` tags + NIP-26 delegation expiry alignment prevent stale manifest replay.

5. **Hash Mismatch**: `manifest_hash` verification prevents MITM delivery payload tampering. Agent runtime MUST reject mismatches before skill execution.

6. **Sybil Attestations**: Zap-weighting prevents zero-cost fake attestations. Trust tier calculation SHOULD incorporate attester reputation and delegation tree distance.

7. **Malicious Importers**: Compatibility mode (§1.5) forces `none` tier for imported skills until author claim verified. Community attestations can raise to `marginal` but not `full` without author involvement.

---

## Acknowledgments

- **Christopher David** (OpenAgents) — NIP-SA architecture, kind number coordination
- **BreezClaw Community** — SKILL.md format, OpenClaw tool patterns
- **Satnam Contributors** — L402 gateway, Cashu bond mechanics, Fedimint integration
- **NIP Authors** — NIP-06 (HD derivation), NIP-26 (delegation), NIP-32 (attestations), NIP-90 (DVMs), NIP-99 (marketplace)

---

## License

This NIP is released into the public domain.

---

## Changelog

**v2 (2026-02-23)**
- Added §2.0 SKILL.md File Format Specification
- Added §2.5 YAML-to-kind:33400 Derivation Algorithm
- Added §2.6 Tool Schema for MCP/OpenClaw/DVM interop
- Added §1.5 Compatibility Mode for BreezClaw/ClaWHub migration
- Added §6.1 Subscription Pricing Tag
- Added §7.0 Two-Event Discovery Pattern (kind:31337 + kind:33400)
- Added `["author_handle"]`, `["l402_endpoint"]`, `["env_required"]`, `["env_optional"]`, `["tool"]` tags
- Added `subscription_period` in price tags
- Formalized trust tier promotion rules with zap thresholds
- Added security considerations for imported skills

**v1 (2026-02-19)**
- Initial draft with core manifest, attestation, and revocation mechanics
