Below is a **build spec + coding-agent instructions** to implement a **custom Storybook-like in-app overlay for Effuse** that loads **CSF (Component Story Format) modules** (CSF 3–style objects), **without React**, using **Vite + TypeScript** (and runnable inside **Tauri**).

---

## 0) Goal

Build `effuse-storybook` (working name): a **dev tool overlay** integrated directly into the main application that:

* **Activates via F12 key press** (toggles visibility).
* Discovers `**/*.stories.ts` (or `.stories.tsx`) via **Vite `import.meta.glob`**.
* Loads each story file as an **ES module**.
* Interprets **default export meta** + **named story exports** per **CSF**.
* Renders stories using **Effuse** (no React) inside a modal/overlay.
* Provides:

  * Left nav: story hierarchy (by `meta.title` or inferred title).
  * Center canvas: rendered story (isolated from main app UI).
  * Right panels:
    * “Controls” for args (simple auto-generated controls).
    * “Actions” log (event log).
    * “Parameters” viewer (JSON).

* Supports core CSF features:
  * `meta`: `title`, `component`, `decorators`, `parameters`, `includeStories`, `excludeStories`
  * story objects: `args`, `render`, `decorators`, `parameters`, `name`, `play`
  * story naming/IDs: export name -> display name (startCase), stable story id from export key unless overridden display `name`
  * default render: when story has no `render`, render `meta.component` with `args`

---

## 1) Constraints / Non-goals

**Constraints**

* **Integration**: Must be part of the main application bundle (or lazy-loaded chunk), not a separate standalone app.
* **Trigger**: Toggled by the **F12** key (or a specific DevTools key combination).
* **Overlay**: Renders on top of the current application UI (z-index overlay), pausing/hiding the main app if necessary.
* **Not React**: Effuse is the renderer.
* **Environment**: Must run in Vite dev and Tauri (webview).

**Non-goals (for v1)**

* Full Storybook addons ecosystem.
* Complex argTypes inference.
* Source code preview / docgen.
* Visual regression testing.
* Separate build process (it builds with the app).

---

## 2) Developer Experience (DX)

### Scripts

* No separate scripts required.
* `bun run dev`: Starts the app. Press **F12** to open the storybook overlay.
* `bun run build`: Builds the app (including the overlay code, potentially stripped in prod if desired, but kept for v1).

### File conventions

* Story files: `src/**/*.stories.ts`
* Default export is required.
* Named exports are stories.

---

## 3) CSF API for Effuse

### 3.1 Type definitions

Create `src/effuse-storybook/csf.ts` (or similar path):

```ts
export type Decorator<Args = any> = (story: StoryFn<Args>, ctx: StoryContext<Args>) => StoryRenderOutput;
export type StoryFn<Args = any> = (args: Args, ctx: StoryContext<Args>) => StoryRenderOutput;

export type StoryRenderOutput =
  | string
  | { html: string; swapMode?: "inner" | "outer" | "morph" } // align with Effuse swap modes
  | Promise<string>
  | Promise<{ html: string; swapMode?: "inner" | "outer" | "morph" }>;

export interface Meta<C = any, Args = any> {
  title?: string;
  component: C; // required by spec
  decorators?: Decorator<Args>[];
  parameters?: Record<string, any>;
  includeStories?: (string | RegExp)[] | RegExp;
  excludeStories?: (string | RegExp)[] | RegExp;
}

export interface StoryObj<Args = any> {
  name?: string;                 // display-only
  args?: Partial<Args>;
  parameters?: Record<string, any>;
  decorators?: Decorator<Args>[];
  render?: StoryFn<Args>;         // optional
  play?: (ctx: PlayContext<Args>) => Promise<void> | void;
}
```

### 3.2 Effuse component contract

Define a convention for `meta.component`:

* `component` is either:
  1. an Effuse “component” function: `(props) => string | { html, swapMode } | Effect`, OR
  2. an object with a `render(props, ctx)` method.

You must normalize it into a `StoryFn` during rendering.

**Default render rule (CSF-like):**
* If story has no `render`, use `meta.component` and pass story args.

### 3.3 Title inference

If `meta.title` is absent, infer title from file path:
* Remove the leading story root (e.g. `src/`)
* Remove `.stories.ts`
* Convert path separators to `/`
* Optionally drop trailing `/index`

Example: `src/components/Button/Button.stories.ts` -> `components/Button/Button`

---

## 4) Story discovery & module loading

### 4.1 Glob + import

Create `src/effuse-storybook/story-index.ts`:

* Use Vite runtime glob:
```ts
const modules = import.meta.glob("/src/**/*.stories.ts", { eager: false });
```
* Build an async index mapping filePath to loader functions.

### 4.2 CSF module parsing

Implement `parseCsfModule(filePath, mod)`:

Inputs:
* `filePath: string`
* `mod: any` (ES module namespace)

Output:
* `CsfFile` containing `title`, `meta`, `stories: CsfStory[]`

Rules:
* `meta = mod.default` (required).
* Determine candidate story export keys.
* Apply `includeStories` / `excludeStories`.
* Compute story identity (`exportName` -> `storyId`).
* Merge parameters/decorators (Meta + Story).

### 4.3 startCase implementation

Implement simple `startCase` for display names (camelCase -> Title Case).

---

## 5) Overlay State Model

Instead of a URL router, use a reactive state machine for the overlay:

* `isOpen`: boolean (toggled by F12)
* `selectedStoryId`: string | null
* `currentArgs`: Record<string, any>

**Optional Deep Linking**:
You *may* check for `?story=...` in the URL on F12 open to restore state, but primary navigation is internal to the overlay.

---

## 6) Rendering pipeline (Effuse integration)

### 6.1 Normalized render function

Create `normalizeRender(meta.component, story.render)`:
* If `story.render` exists, use it.
* Else create adapter to call `meta.component` with args.

### 6.2 Decorator application

Decorators wrap the story function **outside-in**.

### 6.3 Mount & update

Create a `CanvasHost` that:
* Has a root DOM element `#sb-canvas` **inside the overlay**.
* On story selection:
  * Computes initial args.
  * Renders once.
  * Runs `play` after render resolves.
* On args change:
  * Re-renders.

### 6.4 Output normalization

Normalize `StoryRenderOutput` into `{ html, swapMode }` and apply using Effuse's swap mechanism.

---

## 7) Controls (Args UI)

### 7.1 Minimal arg schema

Infer controls from `currentArgs` values (string, number, boolean, object).
Allow explicit `parameters.controls` configuration.

### 7.2 Two-way binding

Controls panel edits `mutableArgs`. On change -> update args -> re-render canvas.
"Reset args" button restores initial story args.

---

## 8) Actions panel (event log)

Implement `action(name)` helper that emits to an `ActionsBus`.
The Actions panel subscribes and prints logs.

---

## 9) Play function support

Provide `PlayContext` (`canvas`, `userEvent`, `expect`) to `play`.
Implement minimal testing helpers (`getByText`, `click`, `type`, `expect`).
Show pass/fail status and errors in the UI.

---

## 10) UI layout (Effuse UI)

Build the overlay UI with Effuse components:

* **Container**: Fixed position, full screen, high z-index, semi-transparent backdrop.
* **Window**: Centered or full-size modal.
  * Left: Sidebar (Tree nav).
  * Center: Canvas (iframe or isolated div).
  * Right: Panels (Controls | Actions | Parameters).

---

## 11) Project structure (integrated)

```
src/
  effuse-storybook/
    index.ts                // exports mountStorybookOverlay()
    Overlay.ts              // Main UI component
    state.ts                // Reactive state (isOpen, selectedStory, args)
    panels/...
    canvas/...
    csf/...
    runtime/...
  main.ts                   // App entry (imports and initializes listener)
```

In `src/main.ts`:
```ts
import { setupStorybookListener } from "./effuse-storybook";

// ... app initialization ...

if (import.meta.env.DEV) {
  setupStorybookListener(); // Listens for F12
}
```

---

## 12) Example story file

`src/components/Button.stories.ts`:

```ts
import { action } from "@/effuse-storybook";
import { Button } from "./Button";

export default {
  title: "components/Button",
  component: Button
};

export const Primary = {
  args: { label: "Primary", onClick: action("clicked") }
};
```

---

## 13) Implementation plan (ordered tasks)

### Task A — CSF parsing + story index
1. Implement `import.meta.glob` discovery.
2. Implement `parseCsfModule` logic.
3. Build story registry.

### Task B — Overlay UI & F12 Trigger
1. Create `setupStorybookListener` to toggle `isOpen` state.
2. Implement `StorybookOverlay` component (hidden by default).
3. Mount overlay to document body on first open.

### Task C — Canvas host + rendering
1. Implement `CanvasHost` inside the overlay.
2. Normalize render function & apply decorators.
3. Render selected story into canvas slot.

### Task D — Controls & Actions
1. Implement Controls panel (infer from args).
2. Implement Actions bus and panel.

### Task E — Play support
1. Implement play runtime and helpers.
2. Run play function on mount.

---

## 14) Testing requirements

* Unit tests for CSF parsing and state logic.
* Manual verification of F12 toggle and story rendering.

---

## 15) “Done” checklist

* ✅ `*.stories.ts` discovered via glob.
* ✅ F12 toggles the Storybook overlay.
* ✅ Sidebar lists stories.
* ✅ Canvas renders selected story.
* ✅ Controls modify args and re-render.
* ✅ Actions log events.
* ✅ Works within the main app dev environment.
