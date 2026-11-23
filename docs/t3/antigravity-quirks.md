# Antigravity 1.5+ (`agy`) Layer-1 Quirk & Parity Radar

- **Last updated:** 2026-11-22
- **T3 Code reference:** `~/work/projects/repos/t3code` (`v0.1.10` `7d32075e37` → `HEAD` `18850a5eeb`, covering all **45** `v0.1.11` Antigravity commits + **22** post-`v0.1.11` Antigravity commits through `18850a5eeb`)
- **OpenAgents baseline:** `origin/main` @ `5e9dc70b08`
- **Role:** Live Layer-1 external-CLI quirk tracker (per [`2026-11-22-t3code-borrowing-theory.md`](2026-11-22-t3code-borrowing-theory.md) §5 & §8). Supersedes the gap matrix in [`docs/audits/2026-11-20-t3code-antigravity-v0-1-11-audit.md`](../audits/2026-11-20-t3code-antigravity-v0-1-11-audit.md), whose 5 phases (`#13831`–`#13839`) are now **shipped**.
- **Companion mobile audit:** [`docs/audits/2026-11-22-mobile-antigravity-over-tailscale-audit.md`](../audits/2026-11-22-mobile-antigravity-over-tailscale-audit.md) (tracks iOS/Tailscale gaps separately from the shared Coax ACP engine).

---

## 1. Status Summary

Every major mechanism from T3 Code `v0.1.11` (`#13831`–`#13839`) is now implemented and tested in OpenAgents' Coax ACP stack (`coax/lib/agent/engine/antigravity.coax`, `coax/lib/agent/engine/cursor.coax`, `coax/lib/agent/cursor/client.coax`, `coax/lib/agent/observe/antigravity.coax`, `coax/lib/agent/engine/agy-install-lib.coax`, `coder/tui/agyworker.coax`, `apps/openagents-desktop/AntigravityProvider.swift`, `apps/openagents-desktop/page/providers_settings.coax`) and shipped in `--minimal` Desktop builds (`bcbd95e454`..`a159b463e7`).

Only a **small set of recent post-`v0.1.11` edge-case fixes** from `t3code` (`d15123668e..18850a5eeb`) remain worth harvesting when triggered, plus mobile Tailscale parity.

---

## 2. Already Shipped in OpenAgents (`origin/main` `5e9dc70b08`)

| Quirk / Capability | T3 Origin | Where It Lives in OpenAgents |
| --- | --- | --- |
| **Runtime `>=1.5.0 <2` version gate & spawn flags** (`--gemini-dir`, relative `--app-data-dir antigravity-acp`, `AGY_CLI_DISABLE_AUTO_UPDATE=1`) | `#38970`, `#39363`, `#37916` | `coax/lib/agent/engine/antigravity.coax` (`AGY_VERSION_PREFIX`, `AGY_APP_DATA_DIR`, `agy_prepare_launch_auth`), `coax/lib/agent/cursor/client.coax` |
| **Managed SHA-512 installer, immutable release dirs & PID-leased pruning** | `#38740`, `#39363`, `#39028`, `#24683` | `coax/lib/agent/engine/agy-install-lib.coax`, `agy-install.coax`, `apps/openagents-desktop/AntigravityProvider.swift` |
| **Per-profile OAuth isolation (`GEMINI_HOME`, `SSH_CONNECTION` keyring bypass, 19-key env strip) & `--login` URL capture** | `#39485`, `#38953`, `#36037`, `#39443`, `#28355` | `antigravity.coax` (`AGY_REMOVED_ENV_KEYS`, `agy_parse_login_response`, `agy_write_settings_json`), `coder/tui/agyworker.coax` (`agyw_login`), `AntigravityProvider.swift` |
| **Auto review mode & escalated `session/request_permission`** (Auto coerces escalated non-deny to `ask`; interactive cards in Desktop) | `#39378` | `antigravity.coax`, `client.coax`, `agyworker.coax`, `apps/openagents-desktop/page/permit.coax` (`024d969d16`, `#13831`) |
| **Background command holds, reactive wakeup grace & `usage_update` settle debounce** (`AGY_BG_HOLD_MS = 30m`, `CUE_BG_WAKE_GRACE_MS = 60s`, `CUE_BG_USAGE_SETTLE_MS = 1500ms`, `Stop` preserves runtime) | `#38864`, `#38916`, `#38899`, `#39989`, `#40858` | `coax/lib/agent/engine/cursor.coax`, `antigravity.coax`, `agyworker.coax` (`a159b463e7`), `page/layout/conversation.coax` ("Running" vs "Ran") |
| **Detached 1.5 subagents & `SUBAGENT_STATE_KILLED` + mid-turn `steers_with_prompt`** | `#39519`, `#39165`, `#39004`, `#38966` | `coax/lib/agent/observe/antigravity.coax` (`agr_subagents_read`, `AGR_SUB_KILLED`), `cursor.coax` (`steers_with_prompt`), `agyworker.coax` |
| **Advertised `/compact` & `available_commands_update`** | `#37020`, `#35621`, `#38955` | `client.coax` (`cu_can_compact`, `available_commands_update`), `agyworker.coax` |
| **Edit-from-here / fresh-session recall rollback** | `#38227` | `agyworker.coax` (`{"recall":"..."}` fresh-session history replay) |
| **Lost Gemini thinking-signature recovery** | `#28889` | `antigravity.coax` (`agy_lost_thinking_signature`), `agyworker.coax` (`retried_sig`) |
| **Sleep-safe stall watchdog & crash auto-respawn** | `#39395`, `#23654` | `cursor.coax` (`CUE_SLEEP_JUMP_MS = 15000`, `cue_watch`), `agyworker.coax` |
| **`thoughtTokens` double-count fix & mid-turn 404 watcher** | `#39686`, `#39869` | `antigravity.coax` (`agy_parse_usage`, `agy_log_upstream_error`) |
| **`call_mcp_tool` display unwrap & `skills.json` parser** | `#39474`, `#39765` | `client.coax` (`call_mcp_tool` unwrap), `antigravity.coax` (`skills.json` parser) |
| **Quota limits probe & Desktop `Settings -> Providers` (Antigravity-only, native monochrome layout, Gemini 4 Neon default, mid-session model/effort switch)** | `#38957`, `#39413`, `#36035` | `antigravity.coax` (`AGY_QUOTA_*`), `AntigravityProvider.swift`, `page/providers_settings.coax` (`f9f6708a03`..`be8f44b460`, `b5b5dbf0ba`, `0d6606e662`) |

---

## 3. Post-`v0.1.11` T3 Antigravity Fixes (`d15123668e..18850a5eeb`) — Disposition

Every one of the 22 Antigravity commits merged into `t3code` after our Nov 20 `v0.1.11` audit, triaged for OpenAgents:

### Harvested in OpenAgents (`#14024`)

| T3 Commit / PR | Bug Fixed in `t3code` | OpenAgents Implementation |
| --- | --- | --- |
| `27ab8d49eb` (`#39478`), `df09ced768` (`#39697`) | Truncating question text or shell command text can split a multi-byte UTF-8 character/surrogate pair. | `cu_copy_utf8` and `cu_truncate_command_head` in `coax/lib/agent/cursor/client.coax`, wired into `cu_tool_display_name` and `agyw_on_permission`. |
| `29a33f8827` (`#39097`) | Attachments whose MIME type includes parameters (e.g., `text/plain; charset=utf-8`) failed attachment type checks. | `agyw_strip_mime_params` in `coder/tui/agyworker.coax`, applied in `agyw_valid_img_mime`, `agyw_build_prompt_blocks`, and `agyw_build_attachment_blocks`. |
| `df67c4f7f9` (`#40702`) | Long shell commands truncated from the front kept only the last 8,000 chars, hiding the actual command name/head. | `cu_truncate_command_head` preserves the command head + `...` in `client.coax` / `agyworker.coax`. |
| `ad1b1f9ca4` (`#40154`) | After server/app restart, `Auto` could disappear from the mode picker until a fresh session advertised `availableModes`. | `ProviderInstancesStore.isVersionAtLeast15` in `apps/openagents-desktop/AntigravityProvider.swift` preserves `Auto` capability across restarts for verified `agy >= 1.5.0`. |
| `ad4d0f0659` (`#39855`) | Transcript tail reads could drop/split steps at the chunk boundary. | `agr_tail_steps` in `coax/lib/agent/observe/antigravity.coax` safely skips partial leading/trailing lines at chunk boundaries without losing the final complete step. |
| `6edb53026b` (`#39806`), `e24f0c7154` (`#39767`) | Scheduled/reactive wake runs could appear finished prematurely or reject mid-wake steers. | Verified `cue_steer` + `agyworker.coax` accept mid-turn steers while `cue_held_bg` is active while preserving active `nbg_tools`. |
| `ece4964b8e` (`#40287`), `367f9d3759` (`#40194`), `26b48d2da8` (`#40020`), `f0ceeefc08` (`#40488`) | `skills.json` edge cases: `null` lists, `~/` entries `agy` never loads, file-listed skills, and literal filter matching. | Ported into `agy_parse_skills_json` / `agy_skill_allowed` (`coax/lib/agent/engine/antigravity.coax`) and `AntigravitySkillsDiscovery` (`apps/openagents-desktop/AntigravityProvider.swift`). |

### Already Covered or T3-Specific (No Action Needed)

| T3 Commit / PR | Summary | Why Done / Not Applicable |
| --- | --- | --- |
| `602e5b32f5` (`#39686`) | Stop double-counting `thoughtTokens` in `outputTokens` | **Already shipped** in `antigravity.coax` (`agy_parse_usage`). |
| `f68d36d8aa` (`#39869`) | Mid-turn 404 shows error & is retryable | **Already shipped** in `antigravity.coax` (`agy_log_upstream_error`). |
| `3a91757430` (`#39989`), `344de5fe8e` (`#40858`) | Keep turns open for pending background wake & bound silent post-settle holds | **Already shipped** in `cursor.coax` / `agyworker.coax` (`a159b463e7`: wake grace + `AGY_BG_HOLD_MS = 30m` + `CUE_BG_USAGE_SETTLE_MS = 1500ms`). |
| `1c379b63ba` (`#39765`) | Discover skills from `skills.json` | **Already shipped** in `antigravity.coax` (`#13837`). |
| `7966f202ef` (`#39822`) | Pre-approve T3 MCP tools in Auto / Auto-accept edits | T3-specific (`t3_*` MCP server tools). |
| `8ff66fc563` (`#41155`), `755de001f2` (`#40352`), `46b626263f` (`#39897`), `e74b795535` (`#39720`) | T3-specific usage aggregation, Orchestrator pricing table, token analytics, and T3 text-gen helper | Not applicable to OpenAgents' minimal core (`usage` is flag-gated off; OpenAgents uses `AntigravityProvider.swift` directly). |

---

## 4. OpenAgents-Side Gaps Outside `t3code`

The biggest remaining Antigravity gaps in OpenAgents are **not** missing T3 Desktop quirks — they are on the **iOS / Tailscale mobile path** documented in [`docs/audits/2026-11-22-mobile-antigravity-over-tailscale-audit.md`](../audits/2026-11-22-mobile-antigravity-over-tailscale-audit.md) (choosing Antigravity/model/effort on phone, routing interactive approvals/questions to phone instead of `no_ask = true`, preserving real tool kinds instead of rewriting everything to `Bash`, streaming/notifications, and unifying the isolated desktop `CODER_AGY_PROFILE_DIR` with phone-started daemon runs).
