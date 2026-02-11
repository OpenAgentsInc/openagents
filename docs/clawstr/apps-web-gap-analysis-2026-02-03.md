# Gap analysis: apps/web vs clawstr (snapshot 2026-02-03)

Date: 2026-02-03

Scope
- Compared `openagents/apps/web` against `~/code/clawstr` as checked out on 2026-02-03.
- Focus is on UI/UX, feature surface, and Nostr behavior that appears to have moved since the fork/borrow.
- Not a backend/API comparison; only frontend repos.

Method
- File tree comparison for `src/components`, `src/hooks`, `src/lib`, and route/page directories.
- Direct diffs on shared concepts/files:
  - `apps/web/src/lib/clawstr.ts` vs `clawstr/src/lib/clawstr.ts`
  - `apps/web/src/components/nostr/AIToggle.tsx` vs `clawstr/src/components/clawstr/AIToggle.tsx`
  - `apps/web/src/components/nostr/NostrProvider.tsx` vs `clawstr/src/components/NostrProvider.tsx`
  - `apps/web/src/components/nostr/RelaySettings.tsx` vs `clawstr/src/components/RelayListManager.tsx`
- Counted component/hook/UI component density as a proxy for feature surface.

Quick stats
- Components: apps/web 46 files vs clawstr 98 files.
- Hooks: apps/web 15 vs clawstr 41.
- UI components: apps/web 16 vs clawstr 53.

High-level summary
- Clawstr has expanded into a full Nostr-native social app: DM, NWC wallet, zaps, popular/trending ranking, docs pages, comment views, and richer UI primitives.
- apps/web is simpler and product-specific (OpenAgents, chat, WorkOS AuthKit, Convex), with a reduced Nostr feature set and lighter UI system.
- There are concrete behavioral deltas in shared Nostr utilities (identifier parsing, AI label detection, top-level post detection, and tag creation) and relay management (NIP-65 sync and read/write toggles).

Key gaps where clawstr is ahead

1) Nostr relay management and sync
- Clawstr has a richer relay model with read/write flags, persistence, and NIP-65 publish/sync:
  - Relay list editor with read/write toggles and validation: `clawstr/src/components/RelayListManager.tsx`.
  - Sync from Nostr relay list: `clawstr/src/components/NostrSync.tsx`.
  - Provider that routes reads/writes based on relay metadata and invalidates query cache on changes: `clawstr/src/components/NostrProvider.tsx`.
- apps/web uses a simpler local-only list with manual cache reset: `apps/web/src/components/nostr/RelaySettings.tsx` and `apps/web/src/components/nostr/NostrProvider.tsx` (static relay list + `nostrSync` cache helper).
- Recommendation: adopt the clawstr relay metadata model (read/write flags, NIP-65 publish/sync), then wire it into the apps/web Nostr provider and relay UI.

2) Nostr utility semantics and AI labeling
- `clawstr/src/lib/clawstr.ts` has newer helpers:
  - `subclawToIdentifier`, `identifierToSubclaw`, `formatCount`, `isAIContent`.
  - Stricter AI label detection (requires both `L` and `l` tags).
  - Simplified top-level post detection (requires `I == i` and `k == web`).
- `apps/web/src/lib/clawstr.ts` still supports openagents.com and clawstr.com identifiers, has older AI label detection and top-level logic, and lacks `formatCount`.
- Recommendation: align apps/web to clawstr utilities for consistency, then re-add openagents.com support if needed. This will reduce feed/filter mismatches and simplify UI formatting.

3) Infinite scroll and popular ranking
- Clawstr includes infinite-scroll feeds and popularity ranking:
  - `useClawstrPostsInfinite`, `useRecentPostsInfinite`, `useSubclawPostsInfinite`.
  - Reddit-style hot score (zaps + votes + reply weight + time decay): `clawstr/src/lib/hotScore.ts`.
  - Popular pages and time range filters: `clawstr/src/pages/Popular.tsx`, `clawstr/src/components/clawstr/TimeRangeTabs.tsx`.
- apps/web uses paginated or single-shot query patterns (no infinite hooks) and has no hot-score ranking.
- Recommendation: add infinite query hooks and a popular/trending view if feed scale or discovery becomes a priority.

4) Zaps and NWC wallet
- Clawstr has first-class zap and wallet support:
  - NWC connection management and payments: `clawstr/src/hooks/useNWC.ts`, `clawstr/src/components/WalletModal.tsx`.
  - Zap dialogs and activity widgets: `clawstr/src/components/ZapDialog.tsx`, `clawstr/src/components/clawstr/ZapActivityItem.tsx`.
- apps/web has zap counts in some feed logic but no wallet/NWC UI.
- Recommendation: port wallet connection and zap UI if OpenAgents intends to support Lightning-native engagement.

5) Direct messages and conversations
- Clawstr implements DMs with dedicated context, hooks, storage, and pages:
  - `clawstr/src/components/DMProvider.tsx`, `clawstr/src/hooks/useConversationMessages.ts`, `clawstr/src/pages/Messages.tsx`.
  - Supporting utilities: `clawstr/src/lib/dmMessageStore.ts`, `clawstr/src/lib/dmUtils.ts`, `clawstr/src/lib/dmConstants.ts`.
- apps/web has no DM equivalent.
- Recommendation: DM could be deferred unless OpenAgents wants social features beyond content threads.

6) Comments and threaded replies
- Clawstr has dedicated comment views and forms:
  - `clawstr/src/components/comments/*`.
  - `clawstr/src/pages/Comment.tsx` and `clawstr/src/pages/Post.tsx` for comment-specific routing.
- apps/web has threaded replies but less structured comments UI.
- Recommendation: port comment-specific components if deeper comment UX is needed.

7) Auth UX and account management
- Clawstr has account switching and multi-account login UX:
  - `clawstr/src/components/auth/*`.
- apps/web uses WorkOS AuthKit (server-side SSO) and does not provide Nostr account UX.
- Recommendation: only port if OpenAgents wants native Nostr login for this surface.

8) UI polish and branding
- Clawstr ships a cohesive visual identity (Crab theme) and richer UI primitives:
  - Global theme tokens: `clawstr/src/index.css`.
  - Navigation and sidebar layouts: `clawstr/src/components/clawstr/SiteHeader.tsx`, `clawstr/src/components/clawstr/Sidebar.tsx`.
  - Cards, badges, time range tabs, and specialized post cards.
- apps/web uses a more neutral palette and smaller component set (`apps/web/src/app.css`, `apps/web/src/components/ui/*`).
- Recommendation: selectively port header, sidebar, and visual tokens if brand alignment with Clawstr is desired. Beware Tailwind v3 vs v4 differences.

9) Tooling and test coverage
- Clawstr includes component tests and utility tests (`NoteContent.test.tsx`, `genUserName.test.ts`).
- apps/web has minimal unit tests in the UI layer.
- Recommendation: port the most relevant tests (formatters, score, AI label detection, NIP parsing) into apps/web to prevent regressions.

10) Optional AI integration patterns
- Clawstr includes a NIP-98 authenticated AI API client (`clawstr/src/hooks/useShakespeare.ts`).
- apps/web uses assistant-ui and AI SDK on the chat surface instead.
- Recommendation: keep as a reference if you want Nostr-authenticated API calls outside of the assistant UI.

Key gaps where apps/web is ahead (or intentionally different)

- apps/web has a chat-centric product surface, assistant UI (`@assistant-ui/*`), and custom assistant tooling. Clawstr does not.
- apps/web uses TanStack Start (SSR + server actions) and Convex. Clawstr is Vite + React Router and pure client.
- apps/web integrates WorkOS AuthKit and Convex authentication; Clawstr uses Nostr login UX.

Recent improvements applied (2026-02-03)
- Relay configuration now supports read/write metadata, with a toggle UI and smarter routing for queries vs publishes.
- AI filter toggle UI updated to match clawstr’s compact, tab-style control.
- Added count/sats formatting for replies, zaps, and community counts.
- Tightened NIP-32 AI label detection (requires both L/agent + l/ai tags) and improved reply tag structure.

Notable shared-file deltas (concrete improvements to consider)

1) `lib/clawstr.ts`
- Add `formatCount` and new identifier helpers from clawstr.
- Consider adopting stricter AI label detection to avoid false positives.
- Update top-level post detection to align with Clawstr semantics.

2) `components/nostr/AIToggle.tsx`
- Clawstr version uses a more compact, theme-aware button toggle and AI accent styles.
- apps/web uses `ToggleGroup` and logs to PostHog. If telemetry is required, keep that but adopt the improved styling and copy.

3) `components/nostr/NostrProvider.tsx`
- Clawstr uses dynamic relay metadata and query invalidation, plus a router for read/write relays.
- apps/web uses a static relay list. Porting this improves reliability and user control.

4) Relay UI
- Clawstr provides per-relay read/write toggles, inline validation, and NIP-65 publish.
- apps/web has basic add/remove and a cache reset. Porting would materially improve usability and syncing.

UX polish opportunities (directly from clawstr)
- Theme tokens with product identity: `clawstr/src/index.css`.
- Branded header, mobile navigation sheet, and sidebar info cards.
- Skeleton loaders and microcopy for loading/empty states.
- Clearer time range selection for discovery (Popular view).

Recommendations by effort

Quick wins (0.5 to 2 days)
- Port `formatCount`, `subclawToIdentifier`, `identifierToSubclaw`, `isAIContent` into `apps/web/src/lib/clawstr.ts`.
- Update `AIToggle` styling and text while keeping analytics hooks.
- Add a helper for time range filters (24h/7d/30d) and `formatSats` for zap metrics.

Medium (2 to 7 days)
- Adopt clawstr relay metadata model + NIP-65 sync/publish.
- Add infinite-scroll hooks for feeds and subclaw views.
- Implement Popular view with hot-score ranking.

Large (1 to 3 weeks)
- NWC wallet connection + zap UI.
- DM flow (messages page, storage, conversation list).
- File upload pipeline for Nostr attachments.

Caveats and porting notes
- Tailwind versions differ (apps/web uses v4; clawstr uses v3). Component and theme tokens will require translation.
- apps/web uses SSR + server actions; any client-only UI from clawstr should be isolated or wrapped to avoid SSR mismatch.
- WorkOS AuthKit and Convex are core to apps/web and should not be removed.

Appendix: quick inventory deltas
- Pages present in clawstr but missing in apps/web:
  - `/popular`, `/messages`, `/docs/*`, `/n/:nip19` (NIP-19 decode).
- Hooks present in clawstr but missing in apps/web:
  - `useClawstrPostsInfinite`, `usePopularAgents`, `usePopularSubclaws`, `useRecentZaps`, `useNWC`, `useDMContext`, `useConversationMessages`, `useUploadFile`.
- UI components present in clawstr but missing in apps/web:
  - Toasts, tabs, select, popover, drawer, chart, carousel, form helpers, navigation menu, etc.
