[![StarCraft: Remastered official promotional image - MobyGames](https://tse4.mm.bing.net/th/id/OIP.dLzreGqOXtyAhF8AOK5lGwHaEo?r=0\&cb=thfvnextfalcon3\&pid=Api)](https://www.mobygames.com/game/107142/starcraft-remastered/promo/group-43740/image-393358/?utm_source=chatgpt.com)

# StarCraft / StarCraft II UI Design Guide

**For replicating the design language in a web or desktop application using Three.js, CSS, and TypeScript**

## 1. Design objective

The StarCraft interface is a **real-time command console**. It is not a neutral productivity UI, and it is not a thin overlay. It makes the user feel like a field commander operating through military, alien, or psionic machinery. The interface must communicate four things at once: spatial awareness, selected-object state, available commands, and strategic economy.

For another application, the goal should not be to copy Blizzard’s assets, names, icons, sounds, or exact panel art. Build an original interface that follows the same design principles: **dense information, strong spatial anchoring, race/faction-themed chrome, tactical feedback, and immediate command execution**.

The original StarCraft manual describes the main interface as a set of stable regions: main screen, resources, minimap, status display, portrait, command buttons, menu, and hide-terrain control. It defines resources as minerals, Vespene gas, and supply; the portrait as the selected unit close-up; the status display as detailed numeric information; the command buttons as actions such as Build and Attack; and the minimap as a battlefield overview that gains detail as the player explores. ([Scribd][1]) StarCraft II keeps the same core mental model but modernizes the presentation with higher-resolution art, richer glow effects, a cleaner command card, broader hotkey support, and more flexible observer/UI tooling. Blizzard’s own StarCraft II controls guide documents the familiar RTS interaction grammar: left-click selection, drag selection, Shift add/remove, Ctrl or double-click unit-type selection, attack-move, right-click movement, stop, hold position, patrol, command queueing, and control groups. ([Blizzard News][2])

---

## 2. Core layout anatomy

Both StarCraft and StarCraft II are built around a **dominant world view** with a **fixed command console**. The user spends most attention in the world, but the console is always available as a decision surface.

### Recommended application layout

| Region                     | StarCraft role                  | Application translation                                                                   |
| -------------------------- | ------------------------------- | ----------------------------------------------------------------------------------------- |
| Main viewport              | Battlefield / world view        | Three.js scene, canvas workspace, dashboard surface, map, simulation, graph, or 3D editor |
| Top-right resource strip   | Minerals, gas, supply           | Global counters: budget, capacity, active jobs, memory, alerts, queue load                |
| Bottom-left minimap        | Battlefield overview            | Scene navigator, document minimap, graph overview, project map, system topology           |
| Bottom-center status panel | Selected unit/building data     | Inspector for selected object, node, file, entity, process, device, or record             |
| Portrait panel             | Identity and emotional feedback | Avatar, live preview, object thumbnail, animated status representation                    |
| Bottom-right command card  | Contextual commands             | 3×3 or 5×3 action grid, shortcuts, quick actions, build/create menu                       |
| Alert / objective region   | Mission updates                 | Notifications, goals, tasks, warnings, tutorial steps                                     |

For StarCraft-inspired UI, anchor the interface to the bottom. The lower console should feel like hardware bolted to the viewport, not like a floating app toolbar.

---

## 3. StarCraft / Brood War UI design language

### 3.1 Overall feel

Original StarCraft has a **chunky, industrial, low-resolution command-console aesthetic**. It feels like the user is issuing commands through a rugged terminal under battlefield conditions. The UI is thick, opaque, and deliberately mechanical.

The most important visual qualities are:

| Quality               | Design behavior                                                                                      |
| --------------------- | ---------------------------------------------------------------------------------------------------- |
| Heavy framing         | Panels have thick bevels, hard edges, inset screens, metal housings, screws, vents, hazard-like trim |
| Low-light contrast    | Black or near-black interior displays surrounded by bright outlines and high-contrast text           |
| Pixel-era density     | Many small controls occupy a compact space; readability depends on contrast and consistent placement |
| Faction skinning      | Terran, Zerg, and Protoss UI surfaces feel materially different                                      |
| Diegetic presentation | The UI resembles in-universe equipment, not an abstract overlay                                      |

The manual’s division of the screen into command console regions is central: the player does not hunt through menus to act; they select an entity, read its state, and issue commands through fixed interface zones. ([Scribd][1])

### 3.2 Terran visual treatment

Terran UI should feel **military-industrial**.

Use dark gunmetal, black glass, brushed steel, warning stripes, red indicator lights, yellow action accents, green terminal graphics, and hard rectangular forms. Borders should be thick and utilitarian. Corners can be chamfered, but not soft. Buttons should look like physical switches or backlit membrane keys.

For a web implementation, Terran styling maps well to CSS gradients, border images, inset shadows, and SVG linework. The visual metaphor is “armored console.”

### 3.3 Zerg visual treatment

Zerg UI should feel **organic, wet, unstable, and living**.

Use carapace shapes, ribbed membranes, dark purples, browns, sickly greens, subtle pulsing, glossy highlights, and asymmetrical curves. Avoid perfect rectangles where possible. Buttons can appear embedded in biological sockets. Selection and warning states can pulse rather than blink mechanically.

For CSS, use pseudo-elements, radial gradients, irregular masks, animated background-position shifts, and SVG filters sparingly. The visual metaphor is “living tissue used as an interface.”

### 3.4 Protoss visual treatment

Protoss UI should feel **ancient, psionic, ceremonial, and advanced**.

Use gold, bronze, deep blue, cyan glow, engraved bevels, crystalline facets, symmetrical ornament, and luminous seams. Buttons should feel like glyphic controls. Information panels should seem powered by light rather than by screens.

For CSS and Three.js, use thin glowing borders, conic/radial gradients, subtle bloom in the 3D scene, and crisp icon geometry. The visual metaphor is “sacred technology.”

---

## 4. StarCraft II UI design language

StarCraft II preserves the RTS layout grammar but makes it more **legible, scalable, and esports-ready**. The command console is still bottom-anchored, but the panels are cleaner, more spacious, and more modular. The command card is visually clearer, the top resource indicators are more polished, and campaign/objective UI can appear as layered holographic panels.

StarCraft II’s interaction model is keyboard-forward. Blizzard’s controls guide emphasizes hotkeys, attack-move, Stop, Hold Position, Patrol, queued commands, autocast, control groups, camera controls, and map pings. ([Blizzard News][2]) A separate Blizzard guide explains that waypoints and queued commands can be issued through the main screen or minimap, and that health bars can be displayed with keys or options. ([Blizzard News][3])

### 4.1 What changed from StarCraft to StarCraft II

| Area                 | StarCraft / Brood War                  | StarCraft II                                                              |
| -------------------- | -------------------------------------- | ------------------------------------------------------------------------- |
| Visual fidelity      | Pixel-art console, heavy opaque panels | Higher-resolution panels, glow, cleaner silhouettes                       |
| Command area         | Compact, chunky command buttons        | More systematic command card, clearer icon hierarchy                      |
| Feedback             | Strong but simple color/icon feedback  | More animated, more readable status and targeting feedback                |
| Observer / replay UI | Secondary to player UI                 | More explicit esports/observer support                                    |
| Custom UI tooling    | Limited compared with SC2              | Editor and patch support for UI elements, observer panels, minimap tuning |

Blizzard’s patch notes for StarCraft II 1.3.0 added observer/replay panels showing player names, race, supply, resource comparisons, army supply, units killed, and APM; they also added simultaneous upper-right resource rows for both players in 1v1 observed games and a hotkey to hide/show the main game UI. ([Blizzard News][4]) Patch 1.5.0 later notes broader UI changes and editor support, including custom high-resolution minimap images, minimap icon scaling/background/color, and trigger dialog item types such as tooltip, unit status, portrait, unit model, offscreen unit, and unit target. ([Blizzard News][5])

For a new application, this means the StarCraft II direction is better when you need a production-ready interface: clearer states, cleaner hierarchy, scalable panels, strong keyboard operation, and customizable overlays.

---

## 5. Functional principles to replicate

### Principle 1: Selection drives the interface

The user selects something in the viewport. The rest of the UI updates around that selection. In StarCraft II, selection can be a single object, a group, or a type-based selection using Ctrl/double-click. ([Blizzard News][2])

In your application, every selectable entity should expose:

| State         | UI expression                                                 |
| ------------- | ------------------------------------------------------------- |
| Identity      | Name, type, icon, portrait/thumbnail                          |
| Health/status | Progress, validity, error state, uptime, readiness            |
| Capabilities  | Command card actions                                          |
| Group context | Multi-selection count, shared actions, mixed-state indicators |
| Location      | Highlight in viewport and minimap                             |

### Principle 2: Commands are contextual, not global

The command card should change based on selection. A worker, building, combat unit, project node, server, document, or simulation entity should expose different actions.

Do not show a giant universal toolbar. Use a stable action grid with contextual contents. This is one of the key StarCraft patterns: a player learns where actions appear, then relies on muscle memory.

### Principle 3: The minimap is not decorative

The minimap is a second navigation surface. StarCraft’s manual describes it as a bird’s-eye view of the battlefield where buildings, units, other players, resources, and explored detail are represented. ([Scribd][1]) In StarCraft II, queued commands and waypoints can also be issued through the minimap. ([Blizzard News][3])

For another application, the minimap should support click-to-pan, drag viewport rectangle, ping/marker placement, and selection visibility. It should also have terrain/detail modes if the workspace is visually dense.

### Principle 4: Resources must be glanceable

Resources are top-level state. In StarCraft, they are minerals, Vespene gas, and supply. ([Scribd][1]) In a business, engineering, simulation, or creative app, resources might be quota, compute, budget, capacity, queue length, active collaborators, render cost, memory, or time remaining.

Keep these indicators short, icon-led, and always visible.

### Principle 5: Keyboard and mouse must reinforce each other

StarCraft II’s control model combines mouse selection with hotkeys, command groups, and queued commands. ([Blizzard News][2]) The UI should always show visible command affordances, but expert users should be able to operate primarily through keyboard shortcuts.

For web tech, this means command-card buttons need both pointer handlers and keyboard bindings. The visible UI should display the hotkey letter in the icon or corner label.

---

## 6. Layout specification for a StarCraft-like application

### 6.1 Desktop-first layout

Use a CSS grid with three major bands:

```css
.app-shell {
  width: 100vw;
  height: 100vh;
  display: grid;
  grid-template-rows: 36px 1fr 220px;
  grid-template-columns: 1fr;
  overflow: hidden;
  background: #05070a;
}

.top-hud {
  grid-row: 1;
  pointer-events: none;
}

.world-layer {
  grid-row: 1 / 3;
  position: relative;
  min-height: 0;
}

.command-console {
  grid-row: 3;
  display: grid;
  grid-template-columns: 260px 1fr 160px 340px;
  gap: 8px;
  padding: 10px 14px 14px;
}
```

Recommended desktop dimensions:

| Element          | Size guidance                             |
| ---------------- | ----------------------------------------- |
| Top resource bar | 32–44 px high                             |
| Bottom console   | 190–260 px high                           |
| Minimap          | 180–240 px square                         |
| Portrait         | 120–160 px wide                           |
| Command card     | 3×3 for simple apps, 5×3 for complex apps |
| Status panel     | Flexible center region                    |

Use `clamp()` so the console scales without consuming the viewport:

```css
:root {
  --console-height: clamp(180px, 24vh, 260px);
}
```

### 6.2 Web responsive behavior

Below roughly 900 px width, do not simply shrink everything. StarCraft-style UI depends on stable regions, so switch to modes:

| Width       | Behavior                                                                   |
| ----------- | -------------------------------------------------------------------------- |
| ≥1200 px    | Full console: minimap, status, portrait, command card                      |
| 900–1199 px | Compact console: smaller portrait, condensed status                        |
| 600–899 px  | Drawer model: minimap and command card visible, status collapses           |
| <600 px     | Mobile command mode: bottom action dock, minimap overlay, inspector drawer |

For desktop web and Electron/Tauri-style apps, prioritize the ≥1200 px layout. This UI style is strongest in landscape orientation.

---

## 7. Component design

### 7.1 World viewport

The Three.js viewport is the “main screen.” It should support selection rings, hover outlines, projected labels, alert pings, and camera movements. Keep UI HTML layered above the canvas rather than building all HUD elements inside WebGL.

Recommended layering:

```txt
.app-shell
  .world-layer
    canvas.three-viewport
    .selection-overlays
    .world-tooltips
    .objective-panel
    .top-hud
  .command-console
```

Use Three.js for world entities and CSS/HTML for the HUD. This gives better text rendering, accessibility, layout control, and responsiveness.

### 7.2 Resource strip

Use compact icon-number pairs. StarCraft-style resource indicators should be readable in less than one second.

Example:

```ts
type ResourceKey = "credits" | "energy" | "capacity" | "alerts";

interface ResourceState {
  key: ResourceKey;
  label: string;
  value: number;
  max?: number;
  trend?: "up" | "down" | "stable";
  warning?: boolean;
}
```

Visual rules:

| State          | Treatment                                     |
| -------------- | --------------------------------------------- |
| Normal         | Small icon, bright number, dark backing plate |
| Low resource   | Amber number, subtle pulse                    |
| Blocked/capped | Red or hazard outline                         |
| Increasing     | Small upward chevron or glow                  |
| Spend preview  | Ghosted negative delta next to current value  |

### 7.3 Minimap

A StarCraft-like minimap should not be just a screenshot. It needs semantic layers:

| Layer                   | Example                                                  |
| ----------------------- | -------------------------------------------------------- |
| Terrain/workspace       | Simplified map, topology, document outline, scene bounds |
| Own entities            | Green/cyan dots or blocks                                |
| External/enemy entities | Red/orange dots                                          |
| Resources/objectives    | Blue, yellow, or purple markers                          |
| Camera rectangle        | White or cyan viewport frame                             |
| Alerts                  | Expanding ping rings                                     |
| Fog/unknown             | Dark mask or muted regions                               |

StarCraft II editor documentation supports minimap image control, including map image or custom image and multiple minimap resolutions. ([StarCraft II Editor Tutorials][6]) Patch notes also document support for high-resolution minimap images and adjustable minimap icon scale/background/color. ([Blizzard News][5]) For your app, this translates to a minimap API rather than a static image.

Example TypeScript model:

```ts
interface MinimapEntity {
  id: string;
  kind: "self" | "ally" | "hostile" | "resource" | "objective" | "alert";
  x: number; // normalized 0..1
  y: number; // normalized 0..1
  size?: number;
  selected?: boolean;
  visible?: boolean;
}
```

### 7.4 Status panel

The status panel is the selected-object inspector. It should answer: “What is this, what condition is it in, and what matters right now?”

For single selection:

```ts
interface SelectionStatus {
  id: string;
  name: string;
  typeLabel: string;
  iconUrl?: string;
  health?: { current: number; max: number };
  energy?: { current: number; max: number };
  tags: string[];
  stats: Array<{ label: string; value: string | number }>;
}
```

For multi-selection:

```ts
interface MultiSelectionStatus {
  count: number;
  groups: Array<{
    typeLabel: string;
    count: number;
    iconUrl?: string;
  }>;
  sharedActions: CommandAction[];
}
```

Visually, show a wireframe, thumbnail, or icon grid on the left and numeric stats on the right. Original StarCraft uses a black status screen with bright outlines and green unit visualization; StarCraft II modernizes this with clearer panel spacing.

### 7.5 Portrait panel

The portrait is an emotional anchor. In StarCraft, it gives the selected unit personality and reinforces that orders are being issued to a specific actor. The manual identifies the portrait as a close-up of the selected unit. ([Scribd][1])

For a non-game application, the portrait can become:

| App type        | Portrait equivalent           |
| --------------- | ----------------------------- |
| 3D editor       | Selected object thumbnail     |
| Monitoring app  | Server/device avatar          |
| AI workflow app | Agent portrait or model state |
| Project manager | Team/person/object card       |
| Simulation app  | Entity preview                |
| Data graph app  | Node preview                  |

Implementation options:

1. Static image thumbnail.
2. Animated SVG portrait.
3. Secondary Three.js render target showing the selected object.
4. CSS sprite/video loop for a “transmission” feel.

### 7.6 Command card

The command card is the most important replication component.

StarCraft II editor tutorials describe buttons being slotted into a unit’s command card and linked to abilities/commands. ([StarCraft II Editor Tutorials][7]) For a web app, treat command-card entries as typed command objects.

```ts
interface CommandAction {
  id: string;
  label: string;
  hotkey?: string;
  icon: string;
  enabled: boolean;
  cooldownMs?: number;
  cost?: Partial<Record<ResourceKey, number>>;
  mode?: "instant" | "target" | "toggle" | "submenu";
  tooltip: string;
  run: () => void;
}
```

Recommended grid:

| Complexity               | Grid                                         |
| ------------------------ | -------------------------------------------- |
| Simple app               | 3×3                                          |
| Complex professional app | 5×3                                          |
| StarCraft II-like        | 15 slots, usually read as 5 columns × 3 rows |
| Touch-focused app        | 2 rows of larger buttons plus drawer         |

Button states:

| State                 | Visual                                   |
| --------------------- | ---------------------------------------- |
| Available             | Bright icon, crisp border                |
| Hover/focus           | Glow, tooltip, hotkey emphasis           |
| Pressed               | Inset shadow, short flash                |
| Disabled              | Darkened icon, visible reason in tooltip |
| Cooldown              | Radial or vertical wipe                  |
| Insufficient resource | Red/amber cost text                      |
| Toggle active         | Persistent rim light                     |
| Submenu               | Corner glyph or nested-frame marker      |

---

## 8. Visual system

### 8.1 Color palette

Do not use one flat palette. Use a base sci-fi palette plus faction skins.

```css
:root {
  --hud-bg: #05080c;
  --hud-panel: #0b1118;
  --hud-panel-2: #111923;
  --hud-line: #4b6478;
  --hud-text: #d7edf8;
  --hud-muted: #7f97a6;
  --hud-good: #47ff86;
  --hud-warn: #ffd34a;
  --hud-danger: #ff4b35;
  --hud-energy: #49c8ff;
}

[data-faction="terran"] {
  --faction-primary: #6f8794;
  --faction-accent: #ffd34a;
  --faction-glow: #45d7ff;
  --faction-panel-material: linear-gradient(#222a31, #090d11);
}

[data-faction="zerg"] {
  --faction-primary: #5b2b61;
  --faction-accent: #7dff5a;
  --faction-glow: #b146ff;
  --faction-panel-material: radial-gradient(circle at 30% 20%, #3b1741, #0a050c);
}

[data-faction="protoss"] {
  --faction-primary: #b6903d;
  --faction-accent: #56d9ff;
  --faction-glow: #7feaff;
  --faction-panel-material: linear-gradient(145deg, #2b2412, #070b16 60%);
}
```

### 8.2 Typography

Use a condensed, technical sans for labels and a monospaced or tabular-number font for counters. Avoid overly decorative sci-fi fonts for body text. The best StarCraft-like typography is readable first, stylized second.

Rules:

| Text type         | Treatment                                       |
| ----------------- | ----------------------------------------------- |
| Resource numbers  | Tabular figures, bright, compact                |
| Unit/object names | Medium weight, all caps or title case           |
| Stats             | Small, high-contrast, aligned columns           |
| Tooltips          | Slightly larger than stats, never tiny          |
| Hotkeys           | Single-letter badge in corner of command button |

### 8.3 Borders and bevels

Use layered borders to create depth:

```css
.panel {
  position: relative;
  background: var(--faction-panel-material);
  border: 1px solid color-mix(in srgb, var(--faction-primary), white 15%);
  box-shadow:
    inset 0 0 0 1px rgba(255,255,255,.08),
    inset 0 -18px 30px rgba(0,0,0,.45),
    0 0 18px rgba(0,0,0,.55);
}

.panel::before {
  content: "";
  position: absolute;
  inset: 3px;
  border: 1px solid rgba(255,255,255,.06);
  pointer-events: none;
}
```

For Terran, use heavy rectangular bevels. For Zerg, use irregular organic masks. For Protoss, use thin luminous inner lines and ornamental corner caps.

### 8.4 Iconography

Use silhouettes, not illustrations. StarCraft command icons are readable because they are bold, symbolic, and color-coded. Your command icons should remain recognizable at 32–48 px.

Guidelines:

| Do                         | Avoid               |
| -------------------------- | ------------------- |
| Use strong silhouettes     | Tiny detailed art   |
| Use consistent perspective | Mixed icon styles   |
| Use hotkey labels          | Hidden shortcuts    |
| Use disabled/cost states   | Silent failure      |
| Use command grouping       | Random button order |

---

## 9. Motion and feedback

StarCraft-style UI should feel responsive but not noisy. Motion should indicate command state, damage/error, alert priority, or live system activity.

Recommended motion patterns:

| Event             | Motion                                      |
| ----------------- | ------------------------------------------- |
| Selection changed | Short scanline sweep or panel refresh       |
| Command issued    | Button depress + viewport confirmation ping |
| Alert             | Expanding ring on minimap and world         |
| Resource blocked  | Brief red/amber pulse on resource strip     |
| Cooldown          | Radial wipe or vertical fill                |
| Autocast/toggle   | Slow persistent glow                        |
| Objective update  | Slide-in panel with hard stop               |

Keep most animation under 160 ms. Use longer loops only for ambient faction flavor, such as Zerg pulsing membranes or Protoss energy flow.

---

## 10. Interaction model

### 10.1 Selection

Implement selection as a first-class state machine.

```ts
type SelectionMode = "none" | "single" | "multi" | "targeting";

interface UIState {
  selectionMode: SelectionMode;
  selectedIds: string[];
  activeCommandId?: string;
  commandTargeting?: {
    commandId: string;
    validTargets: string[];
  };
}
```

Basic behavior:

| Input                     | Behavior                               |
| ------------------------- | -------------------------------------- |
| Click entity              | Select single                          |
| Drag rectangle            | Select multiple                        |
| Shift-click               | Add/remove                             |
| Double-click / Ctrl-click | Select same type in view               |
| Escape                    | Cancel targeting or clear transient UI |
| Number key                | Select control group                   |
| Ctrl + number             | Assign group                           |
| Shift + number            | Add to group                           |

These patterns mirror the RTS control vocabulary documented in Blizzard’s StarCraft II control guide. ([Blizzard News][2])

### 10.2 Command execution

Commands should support three modes:

| Mode     | Example                              | Web-app equivalent                      |
| -------- | ------------------------------------ | --------------------------------------- |
| Instant  | Stop, cancel, repair toggle          | Run action immediately                  |
| Targeted | Move, attack, build placement        | Select destination/object after command |
| Submenu  | Build structures, advanced abilities | Open nested command card                |

For targeted commands, the cursor, world hover state, command card, and tooltip should all enter a targeting mode. This is crucial: the UI should make it obvious that the next click will be interpreted differently.

### 10.3 Queued commands

StarCraft II supports queued commands via Shift. ([Blizzard News][2]) In another application, this is powerful for workflows.

Examples:

| App         | Queued command use                                 |
| ----------- | -------------------------------------------------- |
| 3D editor   | Move camera to A, select object B, run operation C |
| Monitoring  | Restart service, wait, run health check            |
| Data app    | Filter, group, export                              |
| AI workflow | Run model, validate output, send to next agent     |
| Simulation  | Move entity, collect resource, return              |

UI treatment: show a small numbered queue above the command card or as ghosted path segments in the world/minimap.

---

## 11. Three.js implementation guidance

### 11.1 Keep HUD in DOM, world in WebGL

Use Three.js for the battlefield/workspace and DOM/CSS for HUD. This gives better text rendering, focus management, keyboard navigation, screen-reader support, and responsive layout.

Recommended stack:

| Layer                  | Technology                                                  |
| ---------------------- | ----------------------------------------------------------- |
| 3D scene               | Three.js                                                    |
| HUD layout             | CSS Grid/Flexbox                                            |
| State                  | TypeScript store, Zustand, Redux, XState, or custom signals |
| Icons                  | SVG sprites or inline SVG                                   |
| Animation              | CSS transitions for HUD, Three.js animation loop for world  |
| Desktop packaging      | Electron or Tauri                                           |
| Rendering optimization | Instancing, object pooling, texture atlases                 |

### 11.2 Project 3D positions into UI overlays

Use projected labels for selected objects and alerts.

```ts
function worldToScreen(
  position: THREE.Vector3,
  camera: THREE.Camera,
  renderer: THREE.WebGLRenderer
) {
  const projected = position.clone().project(camera);
  const rect = renderer.domElement.getBoundingClientRect();

  return {
    x: (projected.x * 0.5 + 0.5) * rect.width,
    y: (-projected.y * 0.5 + 0.5) * rect.height,
    visible: projected.z >= -1 && projected.z <= 1
  };
}
```

### 11.3 Selection rings

Use mesh rings, decals, or sprites under selected objects. StarCraft selection circles are essential because they connect viewport objects to the status panel.

For Three.js:

| Technique         | Use when                      |
| ----------------- | ----------------------------- |
| RingGeometry mesh | Flat ground selection         |
| SpriteMaterial    | Billboarding selection marker |
| InstancedMesh     | Many selected entities        |
| ShaderMaterial    | Animated glow/pulse           |
| CSS overlay       | 2D editor or non-3D scene     |

### 11.4 Minimap rendering approaches

Choose one:

| Approach                               | Pros                            | Cons                            |
| -------------------------------------- | ------------------------------- | ------------------------------- |
| SVG minimap                            | Easy interaction, crisp markers | Not ideal for huge dynamic maps |
| Canvas 2D minimap                      | Fast, flexible                  | More manual hit testing         |
| Secondary Three.js orthographic render | Accurate scene representation   | More GPU cost                   |
| Hybrid static image + semantic markers | Best for apps                   | Requires coordinate mapping     |

For most web apps, use SVG or Canvas 2D for minimap markers and a static or periodically rendered background.

---

## 12. CSS implementation pattern

A StarCraft-style command card:

```css
.command-card {
  display: grid;
  grid-template-columns: repeat(5, 56px);
  grid-template-rows: repeat(3, 56px);
  gap: 6px;
  padding: 10px;
  background: #030507;
  border: 1px solid var(--faction-primary);
  box-shadow: inset 0 0 20px rgba(0,0,0,.8);
}

.command-button {
  position: relative;
  border: 1px solid color-mix(in srgb, var(--faction-primary), white 20%);
  background:
    radial-gradient(circle at 50% 20%, rgba(255,255,255,.08), transparent 40%),
    linear-gradient(#17202a, #05080b);
  color: var(--hud-text);
  cursor: pointer;
}

.command-button:hover,
.command-button:focus-visible {
  outline: none;
  box-shadow: 0 0 12px var(--faction-glow);
}

.command-button[disabled] {
  cursor: not-allowed;
  filter: grayscale(.8) brightness(.45);
}

.command-hotkey {
  position: absolute;
  right: 3px;
  bottom: 2px;
  font-size: 10px;
  color: var(--faction-accent);
}
```

A resource strip:

```css
.resource-strip {
  position: absolute;
  top: 8px;
  right: 12px;
  display: flex;
  gap: 14px;
  padding: 5px 10px;
  background: rgba(0,0,0,.45);
  border: 1px solid rgba(255,255,255,.12);
  backdrop-filter: blur(4px);
}

.resource {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-variant-numeric: tabular-nums;
  color: var(--hud-text);
}
```

---

## 13. TypeScript architecture

Model the UI as selection-driven state plus command providers.

```ts
interface Entity {
  id: string;
  type: string;
  name: string;
  faction?: "terran" | "zerg" | "protoss";
  position: { x: number; y: number; z?: number };
  status: SelectionStatus;
}

interface CommandProvider {
  canHandle(entity: Entity): boolean;
  getCommands(entity: Entity, state: AppState): CommandAction[];
}

interface AppState {
  entities: Record<string, Entity>;
  selectedIds: string[];
  resources: ResourceState[];
  controlGroups: Record<number, string[]>;
}
```

A simple command provider:

```ts
const serverCommandProvider: CommandProvider = {
  canHandle: entity => entity.type === "server",

  getCommands: entity => [
    {
      id: "restart",
      label: "Restart",
      hotkey: "R",
      icon: "restart.svg",
      enabled: entity.status.tags.includes("online"),
      mode: "instant",
      tooltip: "Restart this server.",
      run: () => restartServer(entity.id)
    },
    {
      id: "inspect",
      label: "Inspect",
      hotkey: "I",
      icon: "inspect.svg",
      enabled: true,
      mode: "instant",
      tooltip: "Open detailed diagnostics.",
      run: () => openInspector(entity.id)
    }
  ]
};
```

---

## 14. Sound and haptics

StarCraft UI feedback is strongly reinforced by sound: command acknowledgments, selection confirmations, alerts, errors, and faction personality. For a non-game app, keep this restrained and optional.

Use sounds for:

| Event            | Sound style                       |
| ---------------- | --------------------------------- |
| Command accepted | Short click/confirm               |
| Command invalid  | Low buzz or muted error           |
| Alert            | Directional ping or warning chirp |
| Selection        | Tiny blip                         |
| Resource blocked | Short warning tick                |

Always provide a mute setting. For desktop apps, subtle haptics can work on supported devices, but do not require them.

---

## 15. Accessibility and usability

A StarCraft-inspired UI can easily become unreadable if treated as pure visual spectacle. Preserve the design language while meeting modern usability expectations.

Key requirements:

| Requirement          | Implementation                                           |
| -------------------- | -------------------------------------------------------- |
| Keyboard access      | Every command-card button focusable and shortcut-enabled |
| Visible focus        | Strong focus ring, not just glow                         |
| Tooltips             | Hover and keyboard focus support                         |
| Contrast             | Text and icons readable over dark panels                 |
| Motion control       | Respect `prefers-reduced-motion`                         |
| Scalable text        | Do not lock all UI text to tiny pixel sizes              |
| Screen reader labels | Buttons need `aria-label`, shortcuts, disabled reasons   |
| Error clarity        | Do not communicate invalid state by color alone          |

Example:

```tsx
<button
  className="command-button"
  disabled={!command.enabled}
  aria-label={`${command.label}${command.hotkey ? `, hotkey ${command.hotkey}` : ""}`}
  title={command.tooltip}
  onClick={command.run}
>
  <img src={command.icon} alt="" />
  {command.hotkey && <span className="command-hotkey">{command.hotkey}</span>}
</button>
```

---

## 16. StarCraft vs. StarCraft II: which style to choose?

Use **StarCraft / Brood War** as the reference when you want a retro, heavy, industrial command-console identity. It is best for apps that should feel tactical, constrained, mechanical, or nostalgic.

Use **StarCraft II** as the reference when you want the same command-console grammar but with better readability, modularity, high-DPI support, and modern professional polish. It is better for serious web/desktop applications where users will work for long periods.

A strong hybrid is often best:
**StarCraft layout discipline + StarCraft II clarity + original faction-inspired styling.**

---

## 17. Practical design checklist

Before implementation, give the designer and engineer this checklist:

| Question                                      | Target answer                                               |
| --------------------------------------------- | ----------------------------------------------------------- |
| What is the “world view”?                     | A Three.js canvas or primary workspace dominates the screen |
| What can be selected?                         | Every major object has identity, status, and commands       |
| Where is global state?                        | Top-right resource strip                                    |
| Where is spatial awareness?                   | Bottom-left minimap                                         |
| Where are actions?                            | Bottom-right command card                                   |
| Where is detailed selection info?             | Bottom-center status panel                                  |
| Are commands keyboard-first?                  | Yes, every command has optional hotkey metadata             |
| Is the UI faction-skinned?                    | Yes, via CSS variables and component variants               |
| Can the user act from the minimap?            | Ideally yes                                                 |
| Are alerts visible in both world and minimap? | Yes                                                         |
| Is the design original?                       | Yes, no copied Blizzard assets or trademarks                |

---

## 18. Final recommendation

Build the application as a **selection-driven command console**. Use Three.js for the interactive world, CSS Grid for the bottom console, SVG/Canvas for the minimap, and TypeScript command providers for contextual actions. Make the interface feel physical and faction-themed, but keep the information hierarchy extremely clear.

The StarCraft UI pattern works because it never loses the user’s chain of command:

**See the world → select an object → read its state → issue a command → receive immediate feedback.**

[1]: https://www.scribd.com/document/31685099/Starcraft-Manual-PC?utm_source=chatgpt.com "Starcraft Manual PC | PDF | Computer Network"
[2]: https://news.blizzard.com/en-us/article/6640645/game-guide-simplified-controls "Game Guide: Simplified Controls — StarCraft II — Blizzard News"
[3]: https://news.blizzard.com/en-us/article/4552955/game-guide-special-control "Game Guide: Special Control — StarCraft II — Blizzard News"
[4]: https://news.blizzard.com/en-us/article/2514162/patch-1-3-0-now-live "Patch 1.3.0 Now Live — StarCraft II — Blizzard News"
[5]: https://news.blizzard.com/en-gb/article/10054522/patch-1-5-0-now-live "Patch 1.5.0 Now Live  — StarCraft II — Blizzard News"
[6]: https://s2editor-guides.readthedocs.io/New_Tutorials/01_Introduction/008_Map_Properties/ "Map Properties - StarCraft II Editor Tutorials"
[7]: https://s2editor-guides.readthedocs.io/New_Tutorials/04_Data_Editor/075_Buttons/?utm_source=chatgpt.com "Buttons - StarCraft II Editor Tutorials - Read the Docs"


