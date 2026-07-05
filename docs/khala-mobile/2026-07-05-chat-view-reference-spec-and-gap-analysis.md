# Khala Mobile Chat View Reference Spec And Gap Analysis

Date: 2026-07-05

Source reference: owner-provided screenshot `IMG_2843.PNG`

## Owner Direction

The mobile thread view should follow the screenshot's structure exactly while
using Khala Mobile's existing OpenAgents color scheme and product vocabulary.
This is a structural target, not a request to copy the screenshot's brand
colors wholesale.

The important screenshot behaviors are:

- Header is a compact floating thread header, not a full-width framed app bar.
- Transcript is mostly plain large text on the dark canvas.
- Tool calls render as one-line summaries, not expanded cards.
- The composer is a bottom floating follow-up bar with a plus action, text
  entry label, mic, and stop/send control.
- Chrome is sparse. The content is the interface.

## Target Structure

### 1. Floating Header

The top safe-area region should look like a native chat detail header:

- Left: circular back button, visually independent from the title block.
- Center: two-line title block.
  - Line 1: thread title, single line, bold, truncates at tail.
  - Line 2: workspace/context subtitle, single line, muted, truncates at tail.
- Right: rounded capsule containing edit/new-note action and overflow action.
- Header floats above the transcript content. It should not use a hard full-width
  divider as the dominant visual separator.

Khala adaptation:

- Use `bg-bg` / near-black canvas.
- Use `text-text` for title and `text-textMuted` for subtitle.
- Use `surfaceRaised` / `surface` for circular/capsule controls with subtle
  `borderMuted`, not bright filled buttons.
- Use OpenAgents/Khala icons or existing primitive equivalents when available;
  avoid text stand-ins except as temporary implementation markers.

### 2. Transcript Rhythm

The transcript should be a vertical reading surface:

- Assistant/status prose is plain text directly on the background.
- Text is large enough for mobile reading, with comfortable line-height.
- There are no message bubbles for ordinary assistant progress updates.
- There are no repeated cards or frames around each update.
- Vertical rhythm alternates between prose blocks and compact tool-call rows.
- The list can begin under the floating header; content may show faintly behind
  the safe-area/header region while scrolling, but readable content must never
  sit under controls at rest.

Khala adaptation:

- Use the existing dark canvas and cool white text.
- Preserve public-safe transcript rules: no raw private sync rows or tokens in
  diagnostics, docs, tests, or receipts.
- Keep rich transcript reduction (`reduceRuntimeTranscript`) as the data source,
  but render most parts with a lighter visual treatment.

### 3. One-Line Tool Call Summaries

Tool calls must collapse to a single tappable row by default:

- Leading icon: small terminal/tool glyph.
- Label: verb plus public-safe target, for example:
  - `Read app-shell.test.ts`
  - `Edited 1 file`
  - `Ran 3 commands`
- Trailing chevron.
- One line only. Truncate target text if needed.
- Muted text color; tool rows are secondary scan landmarks.
- No card body, no status badge by default, no multi-line error/detail unless
  the row is expanded.

Expansion behavior, for a later implementation pass:

- Tapping a tool row may reveal details in place.
- Expanded details must stay bounded and public-safe.
- Failure/error details should use danger text but still avoid a heavy card
  unless there is actionable content.

Khala adaptation:

- Use `text-textMuted` / `text-textFaint` for collapsed rows.
- Use `accent` only for active/running affordances or focus, not every tool row.
- Tool summaries should be derived from transcript tool events:
  - read/open/list/search operations -> `Read <target>`
  - edit/write/apply-patch operations -> `Edited <count> file(s)`
  - shell/batch execution -> `Ran <count> command(s)`
  - unknown tool -> concise `<Tool name>` fallback
- Summary text must be computed by a pure helper so it can be tested without a
  simulator.

### 4. Composer

The composer should match the screenshot's bottom bar structure:

- Floating pill anchored above the home indicator/safe-area bottom.
- Left plus button.
- Center text input with placeholder/label such as `Follow up`.
- Right mic action.
- Rightmost circular send/stop control.
- The active run stop state should be obvious and reachable, but not visually
  overbuilt.
- Composer sits over the transcript with a dark translucent/surface treatment.

Khala adaptation:

- Replace the current multi-control bottom rail with a single composer shell.
- Preserve functional requirements:
  - idle send starts a turn
  - active-turn follow-up can steer or queue
  - stop remains reachable while a turn is active
  - push-to-talk remains available when native readiness allows it
  - lane choice remains supported, but should move out of the default composer
    chrome or become a compact secondary affordance, not a permanent row above
    the input.
- Rounded pill is acceptable here because the screenshot's composer is a
  familiar chat affordance. Do not carry that radius into cards or list rows.

### 5. Scroll Affordances

The screenshot includes a small centered down-arrow affordance above the
composer:

- Show only when the transcript is not pinned to bottom or new output arrives
  below the viewport.
- Use a small circular surface with muted border.
- Tapping scrolls to the latest transcript item.
- Do not show permanently while already at bottom.

## Current Gap Analysis

### `ThreadMessagesScreen`

File: `clients/khala-mobile/src/screens/thread-messages-screen.tsx`

Current state:

- Uses `AppHeader showBack`, which renders a full-width header with border.
- Transcript list uses `contentContainerClassName="gap-2 px-4 py-4"`.
- Empty/error/loading states are centered text blocks using raw `Text`.
- Basic chat messages render as rounded bordered cards.
- Rich transcript rows are delegated to `TranscriptPartRow`, wrapped in
  `Animated.View`, `TouchablePopupHandler`, and `SwipeableItem`.
- A bottom `KeyboardAvoidingView` contains `ChatComposer`.

Gaps:

- Needs a thread-specific floating header instead of generic `AppHeader`.
- Needs transcript rendering primitives that default to plain prose, not cards.
- Needs bottom padding/insets sized for the floating composer.
- Needs scroll-to-latest affordance state.
- Needs raw `Text` replacements with Khala primitives for consistency.

### `TranscriptPartRow`

File: `clients/khala-mobile/src/components/transcript-part-row.tsx`

Current state:

- `text` parts render inside rounded bordered `bg-surfaceRaised` cards.
- `reasoning` parts render inside bordered cards with labels.
- `tool` parts render inside rounded bordered cards; in-flight tools get
  `BackgroundGradient`.
- Tool label includes an emoji-like glyph and separate status text.
- `turn-status` parts render centered divider text plus lane badge and optional
  handoff button.

Gaps:

- Text parts should render as plain large transcript text.
- Tool parts need collapsed one-line summaries by default.
- Tool status should not be the main row structure; status can influence icon,
  tone, or expanded details.
- Reasoning should be quieter and optionally collapsed, closer to prose rhythm
  than a card.
- Turn status rows should be compact separators, not visual interruptions.
- Emoji-like glyphs should be replaced with a proper icon/catalog glyph or a
  minimal existing symbol only as a temporary implementation fallback.

### `ChatComposer`

File: `clients/khala-mobile/src/components/chat-composer.tsx`

Current state:

- Composer is a full-width bottom rail with top border.
- Idle lane picker is a visible row above the input.
- Input is a rounded rectangle inside the rail.
- Mic is an `ArwesButton`.
- Send/Stop are `ArwesButton` square controls.
- Active-turn status appears as a small `BackgroundGradient` label.
- Steer/Queue follow-up picker expands as a row of buttons.

Gaps:

- Needs a single floating pill composer shell.
- Needs plus, text, mic, and stop/send controls in one row.
- Lane selection needs to move out of the default visual path or become a
  compact secondary control.
- Steer/Queue should be redesigned so it does not explode into a busy control
  row during ordinary follow-up typing.
- Arwes/Skia controls should be reserved for high-signal moments; the composer
  should look like a native chat input first.
- Placeholder should read like the screenshot (`Follow up`) for active/run
  contexts and remain short.

### Current Primitive Coverage

Recent Ignite-borrowed primitives help but do not finish this target:

- `KhalaText`, `KhalaButton`, `KhalaScreen`, `KhalaTextField`,
  `KhalaListItem`, and `KhalaEmptyState` cover ordinary UI.
- The chat transcript still has bespoke card-like rendering in
  `TranscriptPartRow`.
- The composer still uses older high-signal Arcade/Arwes primitives in the
  default path.

## Proposed Implementation Slices

1. Add pure transcript summary helpers.
   - `summarizeToolPart(part)` for one-line tool rows.
   - Unit tests for read/edit/run/unknown summaries and public-safe truncation.

2. Add chat-view primitives.
   - `KhalaThreadHeader`
   - `KhalaToolCallRow`
   - `KhalaTranscriptText`
   - `KhalaScrollToLatestButton`
   - Keep these dark, minimal, and border-light.

3. Rework `TranscriptPartRow`.
   - Text becomes plain transcript prose.
   - Tool becomes collapsed one-line row.
   - Reasoning/turn-status become quiet inline transcript elements.
   - Keep swipe-to-quote and copy popup behavior.

4. Rework `ChatComposer`.
   - Replace full-width rail with floating pill.
   - Keep plus / follow-up input / mic / stop-send layout.
   - Preserve stop, push-to-talk, steer/queue, lane targeting, and quote merge
     behavior with focused unit/component tests.

5. Rework `ThreadMessagesScreen`.
   - Swap generic app header for thread floating header.
   - Add bottom inset and scroll-to-latest affordance.
   - Replace raw fallback `Text` with Khala primitives.
   - Record a fresh simulator screenshot/receipt after implementation.

## Acceptance Criteria

- The first visible chat screen matches the screenshot's structure:
  floating header, plain transcript prose, one-line tool rows, floating pill
  composer.
- Tool calls are one-line collapsed rows by default.
- Ordinary assistant progress text is not boxed.
- Composer has the same left-to-right structure as the reference.
- All new summary/formatting logic is unit-tested.
- Existing behavior remains intact: copy, quote, handoff, stop, push-to-talk,
  active-turn lane targeting, and queued/steered follow-ups.
- No credentials, tokens, chat bodies, screenshots with private content, raw
  sync rows, or local machine secrets are added to docs/tests/receipts.

## Non-Goals

- Do not copy the screenshot's exact black/gray palette; use Khala's existing
  dark OpenAgents tokens.
- Do not introduce EAS, hosted CI, or remote build lanes.
- Do not remove the data model or rich transcript reducer.
- Do not flatten security/privacy boundaries to make a prettier transcript.
