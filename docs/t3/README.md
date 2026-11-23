# T3 Code Reference & Borrowing Hub

- **Last updated:** 2026-11-22
- **T3 Code baseline:** `~/work/projects/repos/t3code` (`pingdotgg/t3code`, `main` @ [`18850a5eeb`](https://github.com/pingdotgg/t3code/commit/18850a5eeb388148fc2b8fa3499bf8e624a1154c), `v0.1.12-nightly.20261123.5124`, +2,121 commits since the 2026-11-13 `0.1.9` audit and +321 commits since the 2026-11-20 audits)
- **OpenAgents baseline:** `origin/main` @ `5e9dc70b08` (plus local `main` `6ca74c39e3`, moving Archived chats toggle to `Settings > General`)

This directory is the **single live entry point** for everything OpenAgents learns from, tracks against, or deliberately rejects from `pingdotgg/t3code`. Older point-in-time audits in `docs/research/t3code/` and `docs/audits/` are retained for historical provenance and cross-links, with status banners pointing back here.

---

## Start Here (Live Docs in `docs/t3/`)

| Doc | Role | When to read it |
| --- | --- | --- |
| [**Borrowing Theory**](2026-11-22-t3code-borrowing-theory.md) | **Decision framework & disposition matrix** | **Read first.** The 5-layer selective borrowing theory, 6-question pre-flight gate, empirical record of what stuck vs. what was reverted, and concrete disposition of every T3-inspired subsystem. |
| [**Architecture & Provider Reference**](reference.md) | **Compact T3 Code `18850a5eeb` technical reference** | Replaces the stale `0.1.9` details in `docs/research/t3code/01`–`08` and `2026-11-20-t3code-comprehensive-gap-analysis.md`. Covers topology, routes/DPoP/peer links/tunnels, Orchestration V2, durable workflows, 11 providers + ACP Registry, 17 MCP toolkits, 8 forges, security invariants, and exact upstream source entry points. |
| [**Antigravity 1.5+ Quirk Radar**](antigravity-quirks.md) | **Layer-1 external CLI parity ledger (`agy >= 1.5.0`)** | Tracks every Antigravity commit from `t3code` `v0.1.10` through `18850a5eeb` (45 in `v0.1.11` + 22 post-`v0.1.11`), separated into what OpenAgents has **already shipped** vs. the small set of remaining upstream fixes worth harvesting. |
| [**Desktop Sidebar Gap Analysis**](2026-11-22-desktop-sidebar-gap-analysis.md) | **Updated Desktop sidebar comparison** | Reflects the post-correction OpenAgents Desktop sidebar (flat chronological 2-line `54pt` cards, pure `#000000` + 1px border, `Working` shelf, 5-chat `Load more`, icon-only footer, `Settings > General → Archived chats`, phone sync) vs. T3 Code Desktop (`18850a5eeb`), distinguishing real gaps from rejected chrome. |

---

## Complete Corpus Index Across the Repository

All 22+ T3 Code–related documents across the repository, their current status, and where the live truth now lives:

### 1. Live Hub (`docs/t3/`)

| File | Status | Summary |
| --- | --- | --- |
| [docs/t3/README.md](README.md) | **Current** | This table of contents and corpus disposition index. |
| [docs/t3/2026-11-22-t3code-borrowing-theory.md](2026-11-22-t3code-borrowing-theory.md) | **Current** | Main synthesis and selective-borrowing policy (`5e9dc70b08` / `18850a5eeb`). |
| [docs/t3/reference.md](reference.md) | **Current** | Consolidated T3 Code `18850a5eeb` architecture, provider, MCP, and source-map reference. |
| [docs/t3/antigravity-quirks.md](antigravity-quirks.md) | **Current** | Layer-1 Antigravity `agy >= 1.5.0` quirk tracker (`v0.1.10..18850a5eeb`). |
| [docs/t3/2026-11-22-desktop-sidebar-gap-analysis.md](2026-11-22-desktop-sidebar-gap-analysis.md) | **Current** | Updated Desktop sidebar comparison aligned with the post-correction OpenAgents sidebar and Borrowing Theory. |

### 2. Dated Audits (`docs/audits/`)

| File | Baseline | Status | Notes |
| --- | --- | --- | --- |
| [docs/audits/2026-11-20-t3code-antigravity-v0-1-11-audit.md](../audits/2026-11-20-t3code-antigravity-v0-1-11-audit.md) | T3 `d15123668e` (`v0.1.11`) | **Completed / Superseded** | All 5 phases (`#13831`–`#13839`) landed on `origin/main`. Live post-`v0.1.11` quirk tracker is [antigravity-quirks.md](antigravity-quirks.md); mobile Tailscale gaps live in [2026-11-22-mobile-antigravity-over-tailscale-audit.md](../audits/2026-11-22-mobile-antigravity-over-tailscale-audit.md). |
| [docs/audits/2026-11-20-openagents-desktop-vs-t3code-desktop-gap-analysis.md](../audits/2026-11-20-openagents-desktop-vs-t3code-desktop-gap-analysis.md) | T3 `b568fcbaa6` vs. OA `90e3f9361a` | **Historical / Superseded** | Spawned the 22-issue `#13890` parity blitz, subsequently gated behind 29 flags (`flags.coax`) and stripped from `--minimal` (`openagents-v1-spec.md`). Roadmap superseded by [Borrowing Theory](2026-11-22-t3code-borrowing-theory.md). |
| [docs/audits/2026-11-20-t3code-comprehensive-gap-analysis.md](../audits/2026-11-20-t3code-comprehensive-gap-analysis.md) | T3 `8cfd2b5211` vs. OA `855654fc81` | **Historical / Superseded** | Whole-product snapshot predating Inky (`#13982`), Psionic Metal 4 / CUDA 150+ tok/s, `--minimal` 29-flag architecture, full Antigravity parity, and T3's +321 newer commits. Superseded by [reference.md](reference.md) and [Borrowing Theory](2026-11-22-t3code-borrowing-theory.md). |
| [docs/audits/2026-11-20-neon-transcripts-and-release-vision-audit.md](../audits/2026-11-20-neon-transcripts-and-release-vision-audit.md) | OA `fbd5558daf` | **Historical (Valid Vision)** | Release-vision counter-audit that coined the **Parity Trap** warning; its warning was validated by the Nov 21–22 `--minimal` reckoning and formalized in [Borrowing Theory](2026-11-22-t3code-borrowing-theory.md). |

### 3. Wave-1 Source Audit (`docs/research/t3code/`, Pinned to `0.1.9` `f313d0a6cb`)

Kept intact with `2026-11-22` update notes because they hold deep per-subsystem explanations with pinned GitHub permalinks; for current `18850a5eeb` facts, use [reference.md](reference.md).

| File | Status | Current replacement / update summary |
| --- | --- | --- |
| [docs/research/t3code/README.md](../research/t3code/README.md) | **Archived Index (annotated)** | Points here and to [reference.md](reference.md). |
| [docs/research/t3code/01-apps-and-connections.md](../research/t3code/01-apps-and-connections.md) | **Archived (annotated)** | Core transport/supervisor model still valid; misses peer linking, loopback preview tunnels/SOCKS5, 30-day DPoP renewal, Connect device approval, and relay `HookInboxObject`. |
| [docs/research/t3code/02-orchestration.md](../research/t3code/02-orchestration.md) | **Archived (annotated)** | Core V2 event/projection/receipt/outbox invariants still valid; outdated on Antigravity rollback (now supported via `#38227`) and misses typed turn results (`#38874`), durable workflows (`#38882`), and multi-repo projects (`#35925`). |
| [docs/research/t3code/03-claude-code.md](../research/t3code/03-claude-code.md) | **Archived (annotated)** | SDK `0.3.321` / CLI `>=2.1.280` core still holds at `18850a5eeb`; updated with idle compaction, shared macOS process-table poll (`#37585`), and resume phantom-wake fix (`#41120`). |
| [docs/research/t3code/04-codex.md](../research/t3code/04-codex.md) | **Archived (annotated)** | `codex app-server` stdio + managed ChatGPT broker still holds at `18850a5eeb`; updated with managed runtime pruning (`#38740`) and full-access prompt fix (`#40773`). |
| [docs/research/t3code/05-other-providers.md](../research/t3code/05-other-providers.md) | **Archived (annotated)** | Outdated on Cursor SDK (`1.2.0` → `1.4.2` patched), Antigravity (`>=1.5.0`, Auto, rollback, subagents), fx full-access/per-thread backend, and OpenCode 2.x (`>=2.0.18`). See [reference.md](reference.md) and [antigravity-quirks.md](antigravity-quirks.md). |
| [docs/research/t3code/06-security-and-reliability.md](../research/t3code/06-security-and-reliability.md) | **Archived (annotated)** | Core failure-mode & credential-audience tables remain gold; updated with DPoP renewal, Connect device approval, `linkOrigin` fencing, `allowsPreviewTunnel`, MCP Events webhooks (`#41141`), and 6 eval suites. |
| [docs/research/t3code/07-openagents-path.md](../research/t3code/07-openagents-path.md) | **Superseded** | Lead thesis (*"carry the execution boundary across, not the TypeScript stack"*) lives on in [Borrowing Theory](2026-11-22-t3code-borrowing-theory.md); `6fe29cc703` inventory and UIKit/JSC sequence are obsolete. |
| [docs/research/t3code/08-source-map.md](../research/t3code/08-source-map.md) | **Superseded** | Live `18850a5eeb` versions and entry points are in [reference.md](reference.md). |
| [docs/research/t3code/09-current-gap-analysis.md](../research/t3code/09-current-gap-analysis.md) | **Superseded** | OpenAgents `097e11c86d` (iOS 1085) snapshot; UIKit/JSC iOS, desktop op 14/15/16 mismatch, and pre-GPU gaps are all obsolete. |

### 4. Other Repository Docs with T3 Code Context or Provenance

| File | Status | Relationship to T3 Code |
| --- | --- | --- |
| [docs/coder/compaction-theory.md](../coder/compaction-theory.md) | **Updated** | Explains structural compaction in Microcoder (now maintenance-mode) and **Inky** (`coax/lib/inky/compact.coax`, current local harness), plus Codex and T3 Code (`18850a5eeb`). Fixed stale `#13729` and `left_out` claims. |
| [coax/native/composer.md](../../coax/native/composer.md) | **Updated (Historical UIKit Contract)** | Records T3 `f313d0a6cb` provenance for builds 1071/1072/1079 and the live `openagents.chat.thread.v1` contract; fixed stale `#346bf1` send color (mono skin `ACTION` button) and noted iOS's move to compiled Metal GPU. |
| [docs/verification/2026-11-13-native-chat-composer.md](../verification/2026-11-13-native-chat-composer.md) | **Immutable Verification Record** | Build 1071 verification record (historical banner added). |
| [docs/verification/2026-11-13-native-markdown.md](../verification/2026-11-13-native-markdown.md) | **Immutable Verification Record** | Build 1072 verification record (historical banner added). |
| [docs/protocol/environments-claude-and-world-editing.md](../protocol/environments-claude-and-world-editing.md) | **Updated Links** | Environments/HOST/ENV/SESS spec; T3 links now point to this hub and [Borrowing Theory](2026-11-22-t3code-borrowing-theory.md). |
| [docs/research/zeron/2026-11-14-current-gap-analysis.md](../research/zeron/2026-11-14-current-gap-analysis.md) | **Historical Snapshot (annotated)** | Sibling Zeron gap analysis (`097e11c86d`); cross-link updated to reference this hub alongside archived `09-current-gap-analysis.md`. |
