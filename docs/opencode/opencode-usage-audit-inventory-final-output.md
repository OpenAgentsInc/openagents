Here is the comprehensive inventory plan for `docs/opencode/` — every surface found across the repo.

---

## Inventory Plan: `docs/opencode/`

### 1. Inference / Khala Integration (the GTM push targets)

| File | Relevance |
|---|---|
| `docs/inference/2026-06-25-khala-inference-gtm-push.md`§151 | **Primary source** — names OpenCode as first ecosystem-tool target, exact config JSON for `opencode.json`, model selector path, "what to test" checklist |
| `docs/inference/2026-06-25-opencode-khala-runbook-and-audit.md` | **Operational runbook** — live compatibility audit, bug #6232 (content array + tool-call fixes), smoke commands, benchmark GYM (benchmarking gym) shape |
| `docs/promises/2026-06-25-khala-inference-push-promise-review.md` | Promise copy gate for OpenCode recipe publication |

### 2. Reference Repo (external)

- **Upstream:** `https://github.com/anomalyco/opencode.git` (branch `dev`, head `c45d1db9a`)
- **Local clone:** `/Users/christopherdavid/work/projects/repos/opencode` — standalone git clone, **not a submodule** of this workspace
- **Local CLI:** `opencode 1.17.9`
- **Source package:** `1.17.10`
- **No `projects/repos/opencode` symlink/submodule** exists inside the workspace
- **No `.opencode/` config directory** in the workspace root

### 3. TUI / Rendering Parity Docs (design references to OpenCode internals)

| File | What it references |
|---|---|
| `docs/tui/2026-06-10-opencode-vs-pylon-tui-audit.md` | Full gap analysis of `packages/opencode/src/cli/cmd/tui/` architecture vs Pylon TUI |
| `docs/tui/2026-06-10-opencode-claude-integration-audit.md` | How opencode uses AI SDK (`@ai-sdk/openai`), `streamText()`, ACP server, auth.json at `~/.local/share/opencode/auth.json` |
| `docs/tui/2026-06-10-pylon-tui-parity-roadmap.md` | Evolving Pylon TUI toward the opencode baseline |
| `docs/tui/2026-06-10-opentui-three-webgpu-renderer-audit.md` | opencode's TUI does NOT use WebGPU/Three.js — CPU cell-painting only |
| `docs/autopilot-coder/2026-06-21-opencode-desktop-harvest-for-verse-coding-overlay-audit.md` | 6 harvests from OpenCode Desktop for the Verse coding overlay |
| `docs/traces/2026-06-24-opencode-shareable-chat-render-audit.md` | OpenCode share-link system (`/share/:id`, `/s/:id` drift, `@opencode-ai/enterprise`) |

### 4. Probe Parity Docs (OpenCode as reference implementation)

| File | What it covers |
|---|---|
| `packages/probe/docs/probe-shell-opencode-parity-audit.md` | BashTool V2 (`BashTool`) parity with opencode's shell tool, truncation, shell denial (fish, nu) |
| `packages/probe/docs/probe-write-edit-tool-opencode-parity.md` | Write/Edit tool parity with opencode V1 (`packages/opencode/src/tool/`) and V2 (`packages/core/src/tool/`) |
| `packages/probe/docs/probe-llm-core.md` | Intentionally smaller than `@opencode-ai/llm` |
| `packages/probe/docs/2026-06-08-gemini-opencode-support-audit.md` | Porting `@opencode-ai/llm` concepts |
| `packages/probe/docs/probe-gemini-backend.md` | API-key resolution follows opencode-compatible order |
| `packages/probe/docs/probe-openagents-run-assignment.md` | OpenCode env name rejection |
| `packages/probe/docs/probe-openagents-google-gemini-provider-account-design.md` | OpenCode-specific env names |
| `packages/probe/docs/2026-06-07-chatgpt-account-linking-openagents-audit.md` | OpenCode-shaped names in product surface, materialization types |

### 5. Pylon Runtime Code (active production code paths)

| File | Lines | What it does |
|---|---|---|
| `apps/pylon/src/opencode-run.ts` | 1-121 | **Shared helper** — spawns `opencode run --format json`, parses JSON event stream, returns text/cost/tokens |
| `apps/pylon/src/agent-runtime-adapter.ts` | 31-278 | **Runtime adapter** — checks `opencodePath` and `opencodePrompt`, runs via `opencode-run.ts`, produces typed artifact/proof/result refs |
| `apps/pylon/src/inventory.ts` | 114-165 | **Backend health** — detects `opencodeInstalled`, creates `backend.opencode.cli` health entry |
| `apps/pylon/src/codex-agent.ts` | 16-247 | **Codex bridge** — SDK import, home dir, readiness |
| `apps/pylon/src/codex-composer.ts` | 12-706 | **Composer** — binary discovery, `codex exec` spawning, stream handling |
| `apps/pylon/src/codex-agent-executor.ts` | 10-336 | **Executor** — thread management |
| `apps/pylon/src/labor.ts` | 22, 224, 290-298 | **Labor market** — `codex exec` commands |
| `apps/pylon/docs/proofs/*.json` (24+ files) | `backend.opencode.cli` | Proof artifacts with backend refs |

### 6. Product Code: `opencode_codex` Runtime String

| Layer | Files | Total matches |
|---|---|---|
| **Schema definition** | `packages/sync-schema/src/index.ts:94` | 1 |
| **Default constant** | `workers/api/src/omni-runs.ts:324` | 1 |
| **SQL migrations** | `0019_agent_runtime_modes.sql` (CHECK constraints + CASE transforms) | 4 |
| **Tests** | 29 files across `apps/web/` and `workers/api/` | 29 |
| **Docs referencing it** | 5 audit/docs files | 11 |
| **Product copy** | `docs/2026-06-03-team-project-rooms.md:196` (example UI: "opencode_codex on shc_vm") | 1 |

### 7. Standalone `codex` Runtime String (not opencode_codex)

| Layer | Scope | Approx. count |
|---|---|---|
| **Type/schema definitions** | `packages/agent-runtime-schema`, `packages/autopilot-control-protocol`, `packages/provider-account-schema`, Pylon types, desktop RPC types | ~50 |
| **Runtime code** | Pylon (executor, composer, labor, CLI, sessions, account mgmt), Autopilot Desktop (UI model, RPC, bridge), reverse-proxy routes (`chatgpt-codex`, `codex-runs`) | ~200 |
| **Tests** | Unit tests across Pylon, Autopilot Desktop, provider-account-schema, qa-runner | ~80 |
| **Infrastructure code** | `cloud/crates/oa-codex-control/` (Rust GCE daemon), `scripts/codex-fleet/` | ~5 files |
| **Product copy** | Desktop README, Pylon package.json scripts, help text | ~10 |
| **Docs/audits** | Launch audits, labor market, transcripts, evidence bundles | ~100 |

### 8. Auth & Safety Surfaces

| Surface | File(s) | Detail |
|---|---|---|
| **`auth.json` redaction** | `packages/provider-account-schema/src/index.ts:363` | Literal `auth.json` → `[REDACTED]` in all logs |
| **`OPENCODE_AUTH_CONTENT` redaction** | Same file:351 | Env var bridge redacted |
| **Public projection gates** | ~30+ files in `workers/api/src/` | Regex includes `opencode_auth_content` as first-class secret |
| **Auth materialization** | `scripts/codex-fleet/fetch-codex-auth.mjs` | Reads OpenCode auth.json → translates to codex-native auth.json (0600 perms) |
| **QA runner redaction** | `apps/qa-runner/src/redaction.ts` | 15-category TraceRedactor with allowlist for `openagents/khala` |
| **API key bridge** | `apps/qa-runner/src/khala-config.ts` | `OPENAGENTS_API_KEY` → Khala; value never printed, source label only |
| **OpenCode local auth file** | `~/.local/share/opencode/auth.json` | Documented with `0o600` in multiple audit docs |

### 9. External Surface References

| URL/service | Files |
|---|---|
| `https://opencode.ai` | `docs/inference/*`, `docs/traces/*`, multiple audit docs |
| `https://opencode.ai/config.json` | Schema URL in config examples |
| `https://opencode.ai/s/{id}` | Share link URL pattern |
| `https://console.opencode.ai` | OpenCode console login surface |
| `@opencode-ai/sdk` / `@opencode-ai/sdk/v2` | SDK package references in audits |
| `@opencode-ai/llm` | LLM core package (Probe references) |
| `@opencode-ai/enterprise` | Share link enterprise package |
| `@opencode-ai/ui` | UI package (pierre worker) |
| `@opentui/core` | OpenTUI native Zig terminal engine |

### 10. Named Files With "opencode" in Filename (14 total)

```
docs/inference/2026-06-25-opencode-khala-runbook-and-audit.md
docs/autopilot-coder/2026-06-21-opencode-desktop-harvest-for-verse-coding-overlay-audit.md
docs/traces/2026-06-24-opencode-shareable-chat-render-audit.md
docs/tui/2026-06-10-opencode-claude-integration-audit.md
docs/tui/2026-06-10-opencode-vs-pylon-tui-audit.md
packages/probe/docs/probe-shell-opencode-parity-audit.md
packages/probe/docs/probe-write-edit-tool-opencode-parity.md
packages/probe/docs/2026-06-08-gemini-opencode-support-audit.md
apps/pylon/src/opencode-run.ts
apps/pylon/docs/probe-port/probe-shell-opencode-parity-audit.md
apps/pylon/docs/probe-port/probe-write-edit-tool-opencode-parity.md
apps/pylon/docs/probe-port/2026-06-08-gemini-opencode-support-audit.md
apps/openagents.com/docs/2026-06-05-opencode-share-link-web-code-audit.md
apps/openagents.com/docs/2026-06-02-chatgpt-codex-account-connection-opencode-openauth-audit.md
```

---

### Proposed `docs/opencode/` Structure

Based on the inventory, we would recommend organizing into these sections:

1. **`README.md`** — what OpenCode is in relation to OpenAgents, glossary (`opencode` the product vs `opencode_codex` the runtime string vs `codex` the OpenAI runtime)
2. **`khala-integration.md`** — distill from the two GTM docs: config recipe, tool-call compatibility, running smoke tests, benchmark GYM shape
3. **`runtime-opencode-codex.md`** — inventory of every `opencode_codex` literal: where it's defined in schema (sync-schema), where it's the default runtime (omni-runs), migration history (0019 SQL), test fixtures to update if renaming, and the UI copy path
4. **`runtime-codex.md`** — inventory of standalone `codex` references: type unions, Pylon adapter/bridge code, Autopilot Desktop UI code, reverse-proxy routes (`chatgpt-codex`, `codex-runs`), infrastructure (`oa-codex-control`), and rename implications
5. **`reference-repo.md`** — how the external opencode repo at `/Users/christopherdavid/work/projects/repos/opencode` is used (read-only reference, parity targets, `opencode-run.ts` helper), and whether a workspace symlink should exist
6. **`safety-redaction.md`** — catalog of every redaction/projection gate that catches `opencode_auth_content`, `auth.json`, `OPENCODE_AUTH_CONTENT`; the auth materialization script; the credential chain from `~/.local/share/opencode/auth.json` → `OPENAGENTS_API_KEY` → Khala
7. **`tui-parity.md`** — distill from the TUI audits: what opencode TUI patterns have been harvested and what remains (command palette, plugin slots, theme tokens, attach mode)
8. **`probe-parity.md`** — distill from probe parity audits: shell tool, write/edit tool, LLM core, env-var conventions, and the parity gap
9. **`product-copy-audit.md`** — every user-facing or agent-facing occurrence of `opencode_codex`/`codex`/`OpenCode` in help text, UI labels, error messages, and README; which should be renamed/replaced as Probe replaces codex

---

Want us to proceed with creating any specific section of this plan? The top priority items from the GTM push doc are 1 (Khala integration recipe), 2 (opencode_codex runtime rename implications), and 6 (safety redaction catalog).
