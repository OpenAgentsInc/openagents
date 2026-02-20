# Chat UI migration plan: v4 → openagents.com

**Status:** Plan only — no implementation yet.
**Sources:** `~/code/v4` (Next.js + Khala) for layout/components; `~/code/v5` for **Berkeley Mono** typography and **colors**.
**Target:** `apps/openagents.com` (Laravel + Inertia + React + Vite).

This doc outlines how to make the openagents.com chat UI look and behave more like the v4 chat UI, and how to adopt Berkeley Mono and v5’s color palette from v5.

---

## 1. Current state comparison

### 1.1 openagents.com (current)

- **Entry:** `resources/js/pages/chat.tsx` — single page component.
- **Layout:** Uses `AppLayout` (sidebar + breadcrumbs). Chat content is in a padded flex column with:
  - Conversation header (title, run status, Refresh).
  - Error alert when present.
  - Scrollable message area (simple bordered box, role label + parts).
  - Collapsible “Run details” (run summary + run events).
  - Inline form: `Input` + “Send” `Button`.
- **Message rendering:** Inline in `chat.tsx`: `renderPart()` for text, reasoning, and tool parts. Plain `whitespace-pre-wrap` for text; `<details>` for tools with input/output/error.
- **Data:** `useChat` from `@ai-sdk/react` with `DefaultChatTransport`; props from Laravel: `conversationId`, `initialMessages`, `runs`, `selectedRunId`, `runEvents`.
- **No:** Markdown, bubble layout, prompt suggestions, scroll-to-bottom button, fixed input bar, typing indicator, thumbs up/down, copy button, dedicated message/tool components.

### 1.2 v4 (reference)

- **Entry:** `app/chat/[id]/page.tsx` — loads `Chat` from `@/components/ui/chat` and `useChat` from `@/lib/useChat`.
- **Layout:** Full-height column; scrollable message area with `max-w-[50rem]` centered content; **fixed bottom input bar** (`fixed bottom-0`, shadow, `max-w-[50rem]`); scroll-to-bottom FAB when not auto-scrolling.
- **Components:**
  - **Chat** (`components/ui/chat.tsx`): Message list + fixed input + welcome/prompt suggestions when empty + scroll-to-bottom button.
  - **MessageList** (`components/ui/message-list.tsx`): Vertical list of `ChatMessage` + `TypingIndicator` when `isTyping`.
  - **ChatMessage** (`components/ui/chat-message.tsx`): Bubble-style messages (user right, assistant left), CVA variants, Markdown, tool invocations, copy button, optional thumbs up/down.
  - **MessageInput** (`components/ui/message-input.tsx`): Autosizing textarea, send (arrow) and stop buttons, optional Model/Agent/Tool selection, optional attachments, interrupt prompt (framer-motion).
  - **PromptSuggestions** (`components/ui/prompt-suggestions.tsx`): “Try these prompts” with clickable suggestion cards.
  - **ToolCall** (`components/ui/tool-call.tsx`): Card-based tool state (partial-call/call/result), repo info, View Parameters/Content/Full Result dialogs.
  - **MarkdownRenderer** (`components/ui/markdown-renderer.tsx`): `react-markdown` + `remark-gfm`, code blocks with copy.
  - **CopyButton**, **TypingIndicator**, **ChatForm** (form wrapper for optional attachments).
- **Hooks:** `useAutoScroll` (scroll-to-bottom + FAB), `useAutosizeTextArea`, `useCopyToClipboard`, `useFocusInput`.
- **Data:** v4 `useChat` wraps `@ai-sdk/react` and syncs with Khala (threads, messages). Message shape: `Message` with `content`, `role`, `parts` (text / reasoning / tool-invocation), `toolInvocations` (legacy).

---

## 2. Target UX / UI to migrate (from v4)

- Centered, max-width message column (e.g. `max-w-[50rem]`).
- Fixed bottom input bar (full width, with shadow; inner content max-width aligned with messages).
- Bubble-style messages: user right, assistant left; distinct styling (e.g. border + bg for user, muted for assistant).
- Markdown in message content (including code blocks with copy).
- Typing indicator while the model is responding.
- Scroll-to-bottom button when user has scrolled up.
- Optional prompt suggestions when the conversation is empty (configurable strings).
- Thumbs up/down on assistant messages (optional; can be no-op or wired later).
- Copy button on message content.
- Tool calls rendered as cards (with state: calling vs result) and optional dialogs for parameters/content/full result.
- Single send control (e.g. arrow icon) and optional stop button during generation.
- Optional autosizing textarea and “Press Enter again to interrupt” behavior (can be phased in).

---

## 3. Berkeley Mono and colors (from v5)

Use **v5** (`~/code/v5`) as the reference for typography and color palette so the chat (and optionally the app) matches v5’s look.

### 3.1 Berkeley Mono

- **What:** Monospace font used for code, labels, and UI accents in v5. Tailwind `font-mono` maps to it.
- **v5 setup:**
  - **Font files:** `public/fonts/BerkeleyMono-Regular.woff2`, `BerkeleyMono-Italic.woff2`, `BerkeleyMono-Bold.woff2`, `BerkeleyMono-BoldItalic.woff2`.
  - **Loading:** In v5, `next/font/local` is used in `app/layout.tsx` with `variable: '--font-berkeley-mono'`. For openagents.com (Vite), load via `@font-face` in CSS or a similar mechanism and set the same variable.
  - **Tailwind:** In v5 `tailwind.config.ts`, `theme.extend.fontFamily.mono` is `['var(--font-berkeley-mono)', 'ui-monospace', 'SFMono-Regular', 'Consolas', 'Liberation Mono', 'Menlo', 'monospace']`.
- **Openagents.com:** Add the four Berkeley Mono woff2 files under `public/fonts/` (or equivalent static asset path). Define `--font-berkeley-mono` via `@font-face` and apply it in the Tailwind theme as the first `font-mono` family so all `font-mono` (code blocks, labels, tool names, etc.) use Berkeley Mono.

### 3.2 Colors (v5)

v5 defines two dark palettes we can reuse:

**Option A — Arwes / teal (v5 `app/globals.css` `.dark`):**
Cyan/teal accent, good for a “product” feel.

- Background: `240 10% 3.92%` (near black).
- Foreground: `0 0% 100%`.
- **Accent/primary:** `195 100% 50%` (cyan) for border, ring, sidebar-primary, etc.
- Sidebar: `200 100% 2%` background, `195 100% 50%` primary.
- Optional extra variables: `--arwes-primary`, `--arwes-primary-dark`, `--arwes-primary-light`, `--arwes-bg-dark`, `--arwes-bg-darker`, `--arwes-accent-yellow/purple/red/green`, `--arwes-text-color`, `--arwes-text-secondary`, frames and alert colors (see v5 `app/globals.css` `.dark` block).

**Option B — Cursor Dark High Contrast (v5 `app/teal-theme.css`):**
Neutral grays with a single cyan accent for focus/charts.

- Background: `0 0% 4%` (#0A0A0A).
- Foreground: `208 29% 88%` (#D8DEE9).
- Muted: `0 0% 16%` (#2A2A2A).
- Ring / chart accent: `191 41% 67%` (#88C0D0).
- Primary: `217 17% 31%` (#434C5E). Destructive: `354 42% 56%` (#BF616A).

**Recommendation:** Choose one of the two for the chat UI (and optionally the whole app). Option A matches v5’s main Arwes/teal theme; Option B is a more restrained “editor” look. Apply the chosen HSL values to openagents.com’s CSS variables (e.g. in the same place you define `--background`, `--foreground`, `--primary`, `--muted`, `--border`, `--ring`, sidebar, etc.) so existing Tailwind tokens (`bg-background`, `text-foreground`, `border-border`, etc.) pick them up.

### 3.3 File changes for font + colors

| Change | Where |
|--------|--------|
| Berkeley Mono woff2 files | Copy from v5 `public/fonts/BerkeleyMono-*.woff2` → openagents.com `public/fonts/` (or `resources/` asset path used by Vite). |
| `@font-face` + `--font-berkeley-mono` | Add in openagents.com’s main CSS entry (e.g. `resources/css/app.css` or equivalent). |
| Tailwind `fontFamily.mono` | In Tailwind config, set first entry to `var(--font-berkeley-mono)` (rest of stack as in v5). |
| Color variables | In the same main CSS, add or override `:root` / `.dark` (or theme class) with the chosen v5 palette (Option A or B above). |

---

## 4. Suggested file changes (openagents.com)

Paths below are under `apps/openagents.com/` unless noted.

### 4.1 New files to add

| File | Purpose |
|------|--------|
| `resources/js/components/chat/chat-container.tsx` | Full-height chat wrapper (optional; can live inside page). |
| `resources/js/components/chat/chat.tsx` | Main chat layout: scrollable message area + fixed input + scroll-to-bottom FAB + empty state (welcome/suggestions). |
| `resources/js/components/chat/message-list.tsx` | Renders list of messages + typing indicator. |
| `resources/js/components/chat/chat-message.tsx` | Single message bubble: role-based alignment and styling, parts (text/reasoning/tool), Markdown, copy, optional rating actions. |
| `resources/js/components/chat/message-input.tsx` | Textarea (optionally autosizing), send/stop, placeholder. Simplified first (no model/agent/tool selectors or attachments). |
| `resources/js/components/chat/prompt-suggestions.tsx` | “Try these prompts” with suggestion buttons. |
| `resources/js/components/chat/tool-call.tsx` | Tool invocation card (calling vs result); optional dialogs for params/content/result. Adapt from v4’s `ToolCall`; map openagents.com tool part shape (e.g. `dynamic-tool` / `tool-*`, `input`, `output`, `errorText`) to props. |
| `resources/js/components/chat/markdown-renderer.tsx` | Markdown + GFM; code block with copy. |
| `resources/js/components/chat/typing-indicator.tsx` | Animated dots. |
| `resources/js/components/chat/copy-button.tsx` | Copy to clipboard + optional “Copied” state (or reuse existing if present). |
| `resources/js/hooks/use-auto-scroll.ts` | `containerRef`, `scrollToBottom`, `handleScroll`, `shouldAutoScroll`, `handleTouchStart`. |
| `resources/js/hooks/use-autosize-textarea.ts` | Optional; for autosizing input. |
| `resources/js/hooks/use-copy-to-clipboard.ts` | Optional; for copy button. |

### 4.2 Files to modify

| File | Changes |
|------|--------|
| `resources/js/pages/chat.tsx` | Replace inline layout and `renderPart` with: (1) import and render the new `Chat` component (or equivalent container). (2) Pass `messages`, `input`, `handleInputChange`, `handleSubmit`, `isGenerating`, `stop`, `suggestions`, etc. (3) Keep conversation header and run details if desired — either above the new chat area or in a sidebar/panel. (4) Map `useChat` from `@ai-sdk/react` (and existing transport) into the same props shape the new `Chat` expects (e.g. `messages` with `parts`). |
| `resources/js/types/*` or equivalent | Add or extend types for chat: message with `id`, `role`, `content`, `parts` (text / reasoning / tool), and optional `toolInvocations` for compatibility. Ensure alignment with AI SDK `UIMessage` and backend payload. |
| Main CSS (e.g. `resources/css/app.css`) | Add `@font-face` for Berkeley Mono (four weights/styles) and set `--font-berkeley-mono`. Add or override `:root` / `.dark` with chosen v5 palette (Section 3.2). |
| Tailwind config | Extend `theme.fontFamily.mono` so the first entry is `var(--font-berkeley-mono)` (see Section 3.1). |
| `public/fonts/` (or static asset path) | Add Berkeley Mono woff2 files copied from v5. |

### 4.3 Dependencies to add (if not present)

- `react-markdown`, `remark-gfm` — for `MarkdownRenderer`.
- `framer-motion` — optional; only if migrating the interrupt prompt or other v4 animations.
- `class-variance-authority` — already in openagents.com; use for chat bubble variants if desired (as in v4 `chat-message.tsx`).

### 4.4 Optional / later

- **Model/Agent/Tool selection:** v4’s `MessageInput` includes `ModelSelection`, `AgentSelection`, `ToolSelection`. Not in scope for a first “looks like v4” pass; add later as config or a separate bar.
- **Attachments:** v4 has file attach and drag-drop; omit initially.
- **Welcome card / auth:** v4 shows different empty state for unauthenticated vs free plan; openagents.com can keep a single empty state or add auth-based variants later.
- **Run details:** Keep current “Run details” collapsible in openagents.com; place it above or beside the new chat UI so behavior is unchanged.

---

## 5. Data and type mapping

- **Messages:** openagents.com already uses `UIMessage[]` with `parts`. Map these to the v4-like `Message` shape expected by the new components: `id`, `role`, `content` (aggregate or first text part), `parts` (text / reasoning / tool-invocation). For tool parts, map `type: 'dynamic-tool'` or `type: 'tool-*'` plus `toolCallId`, `state`, `input`, `output`, `errorText` into the structure expected by `ToolCall` (e.g. `ToolInvocation` with `state: 'call' | 'result'`, `toolName`, `args`, `result`).
- **Streaming:** Keep using existing `useChat` and transport; drive `isGenerating` from `status === 'submitted' || status === 'streaming'` and pass to the new `Chat`/message list/typing indicator.
- **Suggestions:** Define a small list of strings in the chat page (or from server props) and pass into `PromptSuggestions`; on click, call `sendMessage` with the suggestion text (and clear input if applicable).

---

## 6. Implementation order (suggested)

1. **Types and deps:** Add message/part types and `react-markdown` + `remark-gfm` (and optional `framer-motion`).
2. **Hooks:** Add `use-auto-scroll` (and optionally `use-autosize-textarea`, `use-copy-to-clipboard`).
3. **Presentational components:** Add `CopyButton`, `TypingIndicator`, `MarkdownRenderer`, then `ToolCall` (adapted to openagents.com part shape), then `ChatMessage`, then `MessageList`.
4. **Input and form:** Add `MessageInput` (simplified: textarea + send + stop) and optionally a thin `ChatForm` wrapper.
5. **Empty state:** Add `PromptSuggestions` and optional welcome copy.
6. **Chat shell:** Add `Chat` that composes: scroll container + `MessageList` + fixed input + scroll-to-bottom FAB + empty state.
7. **Page wiring:** Refactor `resources/js/pages/chat.tsx` to use the new `Chat`, pass props from existing `useChat`, and retain conversation title + run details where appropriate.
8. **Typography and colors:** Add Berkeley Mono (Section 3.1) and v5 color variables (Section 3.2); apply in chat and optionally app-wide.
9. **Polish:** Match v4 spacing (e.g. `pb-28 md:pb-16 pt-20`), max-width, and fixed input height so the “look” matches v4.

---

## 7. v4 source reference (quick index)

| What | v4 path |
|------|--------|
| Chat page | `app/chat/[id]/page.tsx` |
| Chat layout + fixed input | `components/ui/chat.tsx` |
| Message list | `components/ui/message-list.tsx` |
| Message bubble | `components/ui/chat-message.tsx` |
| Input (textarea, send, stop) | `components/ui/message-input.tsx` |
| Suggestions | `components/ui/prompt-suggestions.tsx` |
| Tool cards | `components/ui/tool-call.tsx` |
| Markdown | `components/ui/markdown-renderer.tsx` |
| Typing indicator | `components/ui/typing-indicator.tsx` |
| Copy button | `components/ui/copy-button.tsx` |
| Auto-scroll | `hooks/use-auto-scroll.ts` |
| Message / part types | `lib/types.ts` (Message, MessagePart, ToolInvocation, etc.) |

### 7.1 v5 source reference (typography & colors)

| What | v5 path |
|------|--------|
| Berkeley Mono font loading | `app/layout.tsx` (localFont, `--font-berkeley-mono`) |
| Font files | `public/fonts/BerkeleyMono-*.woff2` |
| Tailwind fontFamily.mono | `tailwind.config.ts` |
| Dark theme (Arwes/teal) | `app/globals.css` (`.dark` block) |
| Cursor Dark High Contrast palette | `app/teal-theme.css` |
| Font config (sans switching) | `lib/config/fonts.ts` (optional; only if switching sans too) |

---

## 8. Out of scope for this migration

- Khala or v4 backend (openagents.com keeps Laravel + existing chat API).
- Model/agent/tool selection UI (can be added later).
- File attachments and drag-drop.
- Auth-specific empty states (can mirror v4 later if needed).
- Any change to Laravel routes or controllers beyond what’s needed to pass suggestion strings or existing props.

This plan is limited to making the openagents.com chat **look and feel** like v4’s chat (layout, bubbles, markdown, tools, input bar, suggestions, scroll behavior) and the **file changes** to get there.
