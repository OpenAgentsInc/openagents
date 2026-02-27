# 2026-02-27 Agent Skills Registry Audit

## Scope

This audit covers:

- Agent Skills protocol/reference implementation in `/Users/christopherdavid/code/agentskills`.
- Current OpenAgents skill-related implementation and docs in `crates/nostr/*`, `apps/autopilot-desktop/*`, and `docs/*`.
- A proposal for a root-level `skills/` registry in this repo that can be used locally now and mapped to the Nostr SKL registry (`kind:33400/33401`) later.

## Method

- Reviewed Agent Skills docs:
  - `/Users/christopherdavid/code/agentskills/docs/specification.mdx`
  - `/Users/christopherdavid/code/agentskills/docs/integrate-skills.mdx`
  - `/Users/christopherdavid/code/agentskills/docs/what-are-skills.mdx`
- Reviewed `skills-ref` reference parser/validator:
  - `/Users/christopherdavid/code/agentskills/skills-ref/src/skills_ref/*`
  - `/Users/christopherdavid/code/agentskills/skills-ref/tests/*`
- Reviewed OpenAgents SKL/SA/AC protocol and app paths:
  - `crates/nostr/core/src/nip_skl/*`
  - `crates/nostr/core/src/nip_sa/skill.rs`
  - `crates/nostr/core/src/nip_ac/scope_hash.rs`
  - `apps/autopilot-desktop/src/runtime_lanes.rs`
  - `apps/autopilot-desktop/src/input/reducers/skl.rs`
  - `docs/PROTOCOL_SURFACE.md`
  - `docs/NIP_SA_SKL_AC_IMPLEMENTATION_PLAN.md`
- Verified current protocol tests:
  - `cargo test -p nostr --tests -q` (pass)

## Current State

### Agent Skills Protocol (external `agentskills` repo)

- Canonical skill unit is a directory containing `SKILL.md`.
- `SKILL.md` requires YAML frontmatter with `name` and `description`.
- Allowed top-level frontmatter fields are limited (`name`, `description`, `license`, `compatibility`, `metadata`, `allowed-tools`).
- Progressive disclosure model is explicit:
  - Load frontmatter metadata for discovery.
  - Load full `SKILL.md` only on activation.
  - Load `scripts/`, `references/`, `assets/` files only as needed.
- `skills-ref` provides:
  - frontmatter parsing
  - validation of naming/shape rules
  - `<available_skills>` prompt generation for agent integration

### OpenAgents (this repo)

- SKL protocol modules are implemented in `crates/nostr/core/src/nip_skl/`:
  - manifest (`33400`)
  - version log (`33401`)
  - optional NIP-90 search profile (`5390/6390`)
  - trust/revocation helpers
  - SKILL payload hash + derivation helper
- SA and AC interop is implemented and tested (`nip_sa/skill.rs`, `nip_ac/scope_hash.rs`, integration tests).
- Desktop includes SKL panes and lane plumbing, but lane behavior is currently simulated/in-memory (event id generation and state transitions), not a filesystem skill registry integration.
- There is currently no root `skills/` directory in this repo.

## Key Gaps

### 1) SKILL frontmatter schema mismatch

OpenAgents `yaml_derivation.rs` currently expects SKL-specific fields like `d`, `version`, `expiry`, and `capabilities` in SKILL frontmatter.  
Agent Skills protocol validation rejects unknown top-level fields beyond the allowed set.

Result: a SKILL file that is valid for current OpenAgents derivation is not necessarily valid Agent Skills protocol, and vice versa.

### 2) No local skills registry structure in repo

There is no project-maintained `skills/` folder convention yet, so there is no canonical place for team-owned skills (for example, Neutron integration skills).

### 3) No registry validation pipeline

OpenAgents does not currently run Agent Skills validation (`skills-ref validate`) across a local skills registry path.

### 4) Docs drift

`docs/NIP_SA_SKL_AC_IMPLEMENTATION_PLAN.md` still states that SKL/AC implementations do not exist, while `crates/nostr/core` now contains implemented modules and tests.

## Proposed Registry Design (Root `skills/`)

Use a project-namespace layout:

```text
skills/
  README.md
  neutron/
    neutron-api-integration/
      SKILL.md
      agents/openai.yaml            # optional, recommended
      scripts/                      # optional
      references/                   # optional
      assets/                       # optional
```

Rules:

- One skill per skill directory (`skills/<project>/<skill-name>/SKILL.md`).
- `SKILL.md` `name` must match `<skill-name>`.
- Keep Agent Skills protocol-compliant top-level frontmatter fields only.
- Put OpenAgents/Nostr bridge metadata under `metadata` keys (namespaced), not new top-level keys.

Suggested metadata keys for bridge mapping:

- `metadata.oa.project`
- `metadata.oa.nostr.identifier` (maps to SKL `d`)
- `metadata.oa.nostr.version`
- `metadata.oa.nostr.expiry_unix`
- `metadata.oa.nostr.capabilities_csv`
- `metadata.oa.nostr.author_npub` (optional)

This keeps files Agent Skills-compatible while carrying enough data to derive SKL manifests.

## Proposed `skills/README.md` Process

`skills/README.md` should define:

1. Directory contract (`skills/<project>/<skill-name>/SKILL.md`).
2. Authoring contract:
   - required frontmatter fields
   - allowed optional fields
   - no extra top-level fields
3. Nostr mapping contract (which `metadata.oa.*` fields are required for SKL publishing).
4. Validation contract:
   - run `skills-ref validate <skill-dir>` for changed skills
   - run registry-wide validation in CI/local script
5. Versioning/release contract:
   - bump `metadata.oa.nostr.version` on behavioral changes
   - publish new `33400`/`33401` entries for released versions

## Implementation Sync Plan

### Phase 1 (docs + structure)

- Add `skills/README.md`.
- Add `skills/<project>/...` directories and first concrete skills.
- Add a simple validation script (for example `scripts/skills/validate_registry.sh`) that runs `skills-ref validate` over all skill directories.

### Phase 2 (protocol bridge alignment)

Update `crates/nostr/core/src/nip_skl/yaml_derivation.rs` to support Agent Skills-compliant input:

- Parse standard Agent Skills frontmatter.
- Read Nostr/SKL bridge fields from `metadata.oa.*`.
- Preserve deterministic payload normalization and hash behavior.
- Keep a compatibility path for legacy SKL-specific frontmatter during migration (optional but recommended).

### Phase 3 (runtime usage)

- Add filesystem discovery for repo-local skills when appropriate.
- Keep desktop SKL lane authority on Nostr events, but source publish inputs from local `skills/` definitions.

## Recommended Next Actions

1. Land this audit.
2. Create `skills/README.md` with the contract above.
3. Add first namespace and skill skeleton (`skills/neutron/...`) using Agent Skills-compliant frontmatter.
4. Implement `yaml_derivation` bridge updates so one `SKILL.md` can satisfy both Agent Skills protocol and OpenAgents SKL publishing.
5. Update stale SKL implementation-plan docs to reflect current implementation reality.
