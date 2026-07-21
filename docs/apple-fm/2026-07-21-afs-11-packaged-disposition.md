# AFS-11 packaged disposition — version-one journey accepted

Date: 2026-07-21
Status: disposition record for GitHub issue #9089
Audience: human and agent

## 1. Outcome

The AFS-11 version-one acceptance journey is **accepted**.

The packaged OpenAgents Desktop application built from `origin/main` at
`fe89a057cb` (includes the #9155 signature-authoritative helper check) reached
**4 agents ready** in the BOOT SEQUENCE, including Apple FM. A host-selected
`codex-local` delegation card moved from **running** to **done**, and the
promoted answer carried the honest `via Codex subagent` attribution.

## 2. Smoke and launch commands

### 2.1 Packaged fixture smoke (Apple FM intentionally off)

Smoke mode disables the live Apple FM launcher. It still proves the packaged
binary boots and tears down cleanly.

```sh
USER_DATA=$(mktemp -d /tmp/oa-afs11-smoke-XXXX)
OPENAGENTS_DESKTOP_SMOKE=1 \
OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF=1 \
OPENAGENTS_DESKTOP_USER_DATA="$USER_DATA" \
OPENAGENTS_DESKTOP_ISOLATED_WORKSPACE_ROOT="$USER_DATA/workspace" \
apps/openagents-desktop/out/OpenAgents-darwin-arm64/OpenAgents.app/Contents/MacOS/OpenAgents
```

Result: `[openagents-desktop smoke] OK` and lifecycle teardown
`{"ok":true,"active":0}`.

### 2.2 Live packaged launch (version-one journey)

```sh
USER_DATA=$(mktemp -d /tmp/oa-afs11-live-XXXX)
OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF=1 \
OPENAGENTS_DESKTOP_USER_DATA="$USER_DATA" \
OPENAGENTS_DESKTOP_ISOLATED_WORKSPACE_ROOT="$USER_DATA/workspace" \
apps/openagents-desktop/out/OpenAgents-darwin-arm64/OpenAgents.app/Contents/MacOS/OpenAgents \
  --remote-debugging-port=9333
```

Build that produced the app:

```sh
pnpm --dir apps/openagents-desktop run package:mac
```

### 2.3 Fixed verify against the stock signed helper

The stock installed app is Developer ID signed and notarized, but its asar still
uses the pre-#9155 digest-first check. The fixed `verifyAppleFmHelper` from main
accepts that same helper when `codesign --verify --strict` passes.

- Helper path: `/Applications/OpenAgents.app/Contents/Resources/native/arm64/foundation-bridge`
- Manifest sha256 and on-disk sha256 **do not** match (codesign rewrote the Mach-O).
- Signature: valid on disk; Designated Requirement satisfied.
- Fixed verify result: **accept** (signature authoritative).

## 3. BOOT SEQUENCE evidence

Scraped from the live packaged renderer after discovery settled:

| Agent | Status | Detail |
| --- | --- | --- |
| Codex | available | gpt-5.6-sol |
| Claude Code | available | claude-fable-5 |
| Grok | available | grok-default |
| Apple FM | available | apple-foundation-model |

Summary line: **4 agents ready** (meets the “3 agents ready” success bar).

## 4. codex-local delegation evidence

UI observations (in order):

1. User message submitted through the Message OpenAgents composer.
2. `DELEGATED AGENTS` showed `0 done · 1 running`.
3. `Codex subagent` card with `data-status="running"` and activity rows.
4. Promoted assistant answer with `via Codex subagent`.

Durable journal (`agent-turns/journal.json`):

| request | selected / effective | state | progressCount |
| --- | --- | --- | --- |
| `request.319f78ef-…` | `provider.apple_fm.local` | completed | 1 |
| `request.codex.96b63b89-…` | `provider.codex.local` | completed | 104 |

Thread store: assistant note `meta.provider = "codex"`, `usageTruth = "exact"`.

Public-safe receipt files:

- `docs/apple-fm/receipts/2026-07-21-afs-11-packaged-disposition/disposition.json`
- `docs/apple-fm/receipts/2026-07-21-afs-11-packaged-disposition/agent-turn-journal.json`
- `docs/apple-fm/receipts/2026-07-21-afs-11-packaged-disposition/threads.public.json`

## 5. Residuals (honest boundary)

1. **Stock stable 0.1.0** at `/Applications/OpenAgents.app` does **not** include
   the #9155 fix in its asar. Its helper is validly signed, but the pre-fix
   runtime still fails with `APPLE_FM_HELPER_DIGEST_MISMATCH`. The next signed
   Desktop release must ship `0919324b30` and `957e646f70`.
2. The package used for this journey was `package:mac` (**adhoc** signature),
   not a new Developer ID notarized DMG. Prior AFS-11 ceremony evidence already
   covers signed+notarized rc.25 / stable packaging. This disposition proves the
   live version-one agent journey on a packaged binary that includes the fix.
3. This record does not re-certify AFS-00/AFS-01 per-turn receipt contracts
   beyond what the earlier evidence boundary already excluded.

## 6. Issue disposition

- Close **#9089** (AFS-11) with this disposition.
- Close epic **#9077** when #9089 closes: it is the last open AFS leaf
  (AFS-00…AFS-10 and AFS-12 are already closed).
