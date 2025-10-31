# Thread Screen ([id].tsx) — Simplification & Typing Audit (2025-10-31)

Author: Audit agent
Scope: Audit and concrete refactor plan for `expo/app/thread/[id].tsx` to simplify logic, remove `any` usage, and improve maintainability and performance.

---

## Executive Summary

The thread screen currently mixes concerns (routing, data merges, rendering, dedupe, provider selection, and scrolling) in a single component, with pervasive `any` casts and ad‑hoc string coercions. It builds a large inlined timeline array and manages scroll with a global ref. This makes the file complex, brittle to change, and hard to type-check.

We can significantly simplify it by:
- Extracting data assembly into a typed hook (`useThreadTimeline`) that merges Tinyvex history with live ACP updates.
- Replacing the `ScrollView` with a typed `FlatList` and a local `ref` (no `globalThis`).
- Moving the provider selector into a small component and unifying haptics into a single, platform‑agnostic call.
- Exporting/centralizing Tinyvex row types and ACP prop types to remove all `any`.
- Replacing route string building with typed `router` calls.

This reduces cyclomatic complexity, eliminates the `any` casts, and gives better separation of concerns while preserving UI/behavior.

---

## Current Issues (by category)

- TypeScript hygiene
  - `useTinyvex() as any` and pervasive `as any` for rows, props, and route navigation.
  - Ad‑hoc `any[]` arrays (e.g., thread rows, tool‑call lists) and `any` content objects (e.g., `{ type: 'text', text } as any`).
  - Hand‑rolled `ToolLike` instead of reusing the app’s `ToolCallLike` type.

- UI composition
  - Large inline IIFE builds `items: { ts, key, render }[]` intermixing Tinyvex and ACP mapping logic.
  - Manual kind/status normalization for tool calls via lowercase substring checks.
  - Route pushes use string concatenation and `as any` casts.

- State/side‑effects
  - Scroll position managed via `(globalThis as any).__threadScroll` and `onContentSizeChange`.
  - Provider selection onPress duplicates identical haptics branches per platform.

- Data ownership
  - Aliasing between client thread id and canonical session id is handled inside the screen; should live in provider/hook.
  - Dedupe of ACP chunks vs Tinyvex history is intertwined with render logic.

---

## Simplification Plan (high‑level)

1) Introduce typed models and exports
- Export Tinyvex types from the provider or move to `expo/types/tinyvex.ts`:
  - `ThreadsRow`, `MessageRow`, `ToolCallRow` (shape already present in provider file; make them exported).
- Ensure `useTinyvex()` returns a typed context (remove `as any`).
- Reuse existing ACP types: `SessionNotificationWithTs`, `ToolCallLike`, `TextContent` from `@/types/acp`.

2) Add a dedicated hook: `useThreadTimeline(threadId)`
- Inputs: `threadId`, `useAcp().eventsForThread`, `useTinyvex()` data accessors (messages, tool calls), and provider’s alias mapping (canonical id lookup hidden behind provider).
- Outputs: `TimelineItem[]` where:
  - `type TimelineItem = { key: string; ts: number; node: React.ReactNode }`.
- Responsibilities:
  - Fetch/subscribe to Tinyvex history (messages tail and tool calls) via provider APIs.
  - Merge ACP updates with Tinyvex, deduping live text/thought chunks when Tinyvex has history for the same time window.
  - Map tool call kinds/status using a typed normalizer: `mapToolKind`, `mapToolStatus`.
  - Omit reasoning messages from Tinyvex timeline (keep them for message detail).

3) Replace `ScrollView` with `FlatList`
- Use `FlatList<TimelineItem>` with:
  - `data={timeline}`; `renderItem={({ item }) => item.node}`; `keyExtractor={(i) => i.key}`.
  - `inverted` or `maintainVisibleContentPosition` for bottom‑anchored behavior (remove global ref and `onContentSizeChange`).
  - Memoized `getItemLayout` if item heights are uniform (optional), or keep default.

4) Extract `ProviderSelector` component
- Props: `{ value: 'codex' | 'claude_code'; onChange: (v) => void }`.
- Single `impactLight()` helper:
  - `export async function impactLight() { try { await Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light) } catch {} }`
  - Call only on value change; no platform duplication.

5) Routing without `as any`
- For new thread: `router.replace({ pathname: '/thread/[id]', params: { id: gen } })`.
- Message detail: `router.push({ pathname: '/thread/[id]/message/[mid]', params: { id: threadId, mid: String(m.id) } })`.

6) Move alias/canonical logic into provider
- Expose `queryMessages(threadId)` and `queryToolCalls(threadId)` that internally handle canonical session id fallback.
- Screen code no longer inspects `threads` array or performs `resume_id` indirection.

---

## Concrete Typing Fixes (remove all `any`)

- useTinyvex
  - Before: `const { messagesByThread, ... } = useTinyvex() as any`
  - After: `const { messagesByThread, ... }: TinyvexContextValue = useTinyvex()` (export the context value type from provider).

- Thread rows, tool calls
  - Before: `const rows: any[] = Array.isArray(threads) ? threads : []`
  - After: `const rows: ThreadsRow[] = Array.isArray(threads) ? threads : []` (but move canon lookup into provider; then remove this block altogether).

- Text content
  - Before: `const content = { type: 'text', text } as any`
  - After: `const content: TextContent = { type: 'text', text, annotations: undefined, meta: undefined }` (or the minimal accepted shape for the component props).

- Tool calls (Tinyvex → UI)
  - Before: custom `ToolLike` and `props as any` with string includes.
  - After:
    - Use `ToolCallLike` as the props type.
    - Add functions:
      - `function mapToolKind(raw?: string): ToolCallLike['kind'] { /* normalize */ }`
      - `function mapToolStatus(raw?: string): ToolCallLike['status'] { /* normalize */ }`
    - Parse locations:
      - `const locations: ToolCallLike['locations'] = parseLocations(r.locations_json)` where `parseLocations` is typed and safe.

- ACP guards
  - Replace `u: any` with discriminated unions via type guards, e.g.:
    - `function isUserMessageChunk(u: unknown): u is { sessionUpdate: 'user_message_chunk'; content: TextContent } { /* check shape */ }`
    - Do the same for `agent_message_chunk`, `agent_thought_chunk`, `plan`, `tool_call`, `tool_call_update`.

- Router calls
  - Remove `as any` by using typed object routes as shown above.

- Refs and scrolling
  - Replace `(globalThis as any).__threadScroll` with `const listRef = React.useRef<FlatList<TimelineItem> | null>(null)` and use `listRef.current?.scrollToEnd()` only when needed; better, use `maintainVisibleContentPosition`.

---

## Proposed File Structure (sketch)

- expo/app/thread/[id].tsx
  - Imports: typed providers, `useThreadTimeline`, `ProviderSelector`, `impactLight`.
  - Local state: threadId (derived from route or generated), provider value from settings.
  - UI: Header title, FlatList for `timeline`, Composer at bottom.

- expo/hooks/use-thread-timeline.ts
  - Exports: `useThreadTimeline(threadId: string): TimelineItem[]`.
  - Internals: merges Tinyvex messages/tool calls with ACP updates; dedupes; returns stable keys.

- expo/components/thread/ProviderSelector.tsx
  - Props and onChange; calls `impactLight()` only on change.

- expo/utils/haptics.ts
  - `export async function impactLight()`.

- expo/types/tinyvex.ts (or export from provider)
  - `export type ThreadsRow`, `MessageRow`, `ToolCallRow`.

---

## Example Pseudocode (core pieces)

Typed router replace/new thread id

```ts
const { id } = useLocalSearchParams<{ id?: string }>()
React.useEffect(() => {
  if (!id || id === 'new') {
    const gen = `t-${Date.now()}`
    router.replace({ pathname: '/thread/[id]', params: { id: gen } })
  }
}, [id])
```

Provider selector without duplicate branches

```tsx
function ProviderSelector({ value, onChange }: { value: 'codex'|'claude_code'; onChange: (v:'codex'|'claude_code')=>void }) {
  return (
    <View style={{ flexDirection: 'row', gap: 8 }}>
      {(['codex','claude_code'] as const).map(p => (
        <Pressable key={p} onPress={async () => { if (value !== p) { await impactLight(); onChange(p) } }}>
          <Text style={{ /* active styles */ }}>{p === 'codex' ? 'Codex' : 'Claude Code'}</Text>
        </Pressable>
      ))}
    </View>
  )
}
```

Timeline assembly (hook)

```ts
type TimelineItem = { key: string; ts: number; node: React.ReactNode }

export function useThreadTimeline(threadId: string): TimelineItem[] {
  const { eventsForThread } = useAcp()
  const { messagesByThread, toolCallsByThread } = useTinyvex()
  const acp = React.useMemo(() => eventsForThread(threadId), [eventsForThread, threadId])
  const msgs = React.useMemo(() => messagesByThread[threadId] ?? [], [messagesByThread, threadId])
  const calls = React.useMemo(() => toolCallsByThread[threadId] ?? [], [toolCallsByThread, threadId])

  // map and merge with strong types; return sorted items
  // ...
}
```

FlatList replacement

```tsx
<FlatList
  ref={listRef}
  data={timeline}
  keyExtractor={(i) => i.key}
  renderItem={({ item }) => <View style={{ paddingVertical: 4 }}>{item.node}</View>}
  contentContainerStyle={{ padding: 16, gap: 10, paddingBottom: 80 }}
  keyboardShouldPersistTaps="handled"
  maintainVisibleContentPosition={{ minIndexForVisible: 0 }}
/>
```

---

## Expected Outcomes

- Zero `any` usage in `expo/app/thread/[id].tsx`.
- No `globalThis` mutations; scroll behavior handled by `FlatList`.
- Provider selection with a single haptics call (no duplicate branches).
- `useTinyvex()` and related structures fully typed and exported where necessary.
- Timeline assembly is testable and reusable via a hook.
- Clearer, smaller screen component (render‑focused; no heavy data assembly).

---

## Acceptance Criteria

- TypeScript: `bun run typecheck` passes with no `any` in the thread screen and no new `any` introduced elsewhere.
- Behavior:
  - New thread auto‑navigates with typed router call.
  - Messages render as before; assistant messages show first line, tap opens detail.
  - Tool calls render with normalized kind/status; Tinyvex backfill still hydrates on demand.
  - Auto-scroll behavior remains sensible without global refs.
  - Provider toggle works with a single haptic feedback on change.

---

## Rollout Steps (incremental, low risk)

1. Export Tinyvex provider types and update `useTinyvex` consumer sites (no behavioral change).
2. Add `impactLight` util and `ProviderSelector` component; replace inline selector in the screen.
3. Implement `useThreadTimeline` and migrate the screen to a `FlatList`.
4. Remove inline IIFE and all `any` casts; add type guards.
5. Verify typecheck and manual flows on iOS/Android/web; adjust minor styling if needed.

---

## Notes

- The `SessionUpdate*` components already accept typed props; aligning construction to those types eliminates many `as any` casts immediately.
- Consider exporting a canonical id resolver from the Tinyvex provider to fully remove alias logic from the screen.

