# Creator experience

Status: proposed design, 2026-10-22. This document specifies interactions; it does not describe a shipped interface.

## World-first workspace

The editor uses React DOM and TypeScript components, Effect Atom for reactive UI
state, and Effect 4 application services. [Frontend UI](frontend.md) defines their
boundaries. The engine owns the fullscreen WebGPU view and advances independently
of component rendering and UI subscriptions.

The world fills the application viewport. Browser fullscreen is an optional explicit action; the normal layout fills its tab without requiring fullscreen permission. The default UI consists of a small project and save indicator at the top left, a selection label near the target, and a bottom-centered hotbar. There is no persistent inspector sidebar.

The hotbar contains **Select**, **Build**, **Assets**, **Behaviors**, **Command**, **Undo**, **Redo**, and **Play**. At ordinary desktop width it occupies one row. A secondary overflow menu contains project, import, publishing, settings, and diagnostics. Labels accompany icons or appear in an accessible named compact mode; unlabeled symbols alone are insufficient. The keyboard shortcut appears in tooltips and the shortcut reference.

Only one tool panel opens at a time. Asset search, room controls, behavior settings, and property editing appear as bounded floating panels above the hotbar. They close after placement when appropriate and retain the last search when reopened. Advanced source, shader, and performance views open on explicit request and can be detached or resized. The default world view must remain available with one escape action. Closing a source tool preserves its unsaved draft and selection; Escape cannot silently discard text or publish it.

A 3D target outline, a readable name, and an optional plan view identify the selection. Color alone must not distinguish editable, blocked, pending, and selected states. Tool panels display exact units and constraints. An add-room preview shows its full footprint and connection to the selected building before committing.

## Interaction modes and focus

**Edit** is the default mode. Clicking selects, dragging moves a supported handle, and a visible shortcut enters walk navigation. **Walk** uses the runtime's collision rules. **Play** starts a named play session from a chosen world generation and keeps gameplay input distinct from editor input. In solo local play, leaving Play restores the editor camera unless the creator chooses **Keep this view**. It does not silently copy runtime movement or spawned entities into the document.

Pointer lock requires a deliberate interaction and always has a visible exit instruction. Losing pointer lock, tab visibility, or window focus clears movement keys and stops camera input. Opening a text field exits lock and suspends game shortcuts. Pointer Lock is a browser-controlled API with its own release gesture and error events; successful acquisition cannot be assumed. [Pointer Lock 2.0 draft](https://www.w3.org/TR/pointerlock-2/).

Initial keyboard bindings are a remappable proposal:

| Action | Default |
| --- | --- |
| Open command field | `/` outside text inputs |
| Select tool | `V` |
| Assets | `A` outside navigation |
| Build | `B` outside navigation |
| Walk and edit toggle | `Tab` while the canvas has navigation focus |
| Walk movement | `W`, `A`, `S`, `D`; arrow-key alternative |
| Undo and redo | Platform-standard modifier plus `Z`, and its redo counterpart |
| Cancel placement or close active tool | `Escape` |

`Tab` retains normal focus navigation whenever DOM controls have focus; entering navigation focus is explicit. Browser and assistive-technology shortcuts take precedence. The editor never handles character shortcuts inside an input, text editor, composition session, or unrelated focused control.

Touch uses tap selection, a placement confirmation control, and an explicit walk mode with independently adjustable movement and look controls. Every drag operation has a tap-and-field alternative. Gamepad and switch interfaces map onto the same action names rather than separate editing logic. Touch controls must not cover the current placement footprint; landscape and portrait layouts both remain operable within the supported platform policy.

## Commands, preview, and feedback

The command field accepts ordinary language, displays recent commands, and offers concrete examples appropriate to the current selection. It shows **Working…** only while a cancellable request is active. Controls remain usable. A manual edit cancels or invalidates a pending semantic request that depends on the old selection or document revision.

Routine reversible local commands apply when their meaning, scope, and authority are resolved. They leave a short receipt such as **Added 5 large pumpkins · Undo**. Topology changes show a placement outline while committing, then the resulting doorway or wall. An atomic transaction either completes or leaves the prior document unchanged. Cheap local preflight rejects an edit that would remove the occupied floor or enclose the player, with a remedy such as **Step outside this room before removing it**. If such a condition is discovered after a shared document commit, activation waits while the accepted edit remains inspectable; the editor cannot rewind another creator's revision. It offers movement, repair, or an explicitly approved safe relocation, and never teleports the creator silently.

If “this wall” matches the current aimed editable surface, the command uses that surface's stable ID. If it matches several genuine candidates, the interface highlights at most three choices with names and locations. It does not return a generic ambiguity error for a fully specified command such as “add five huge pumpkins to my left.” Exact counts, supported assets, size presets, and captured direction resolve through [AI authoring](ai-authoring.md).

Compilation uses visible states: **Checking**, **Updating**, or **Couldn't apply this edit**. Detailed errors belong beside the affected shader, script, or field. The interface marks **Showing previous version** if the document and runtime differ. A completed save uses **Saved on this device** or **Saved to project** according to actual durability. Network interruption changes this label immediately rather than implying cloud persistence.

Undo describes the action it reverses. Collaborative undo is a new validated compensating transaction and can fail if later work depends on the target. The interface then explains the dependency and offers an isolated branch; it never destroys another creator's work to recreate an old snapshot. See [Collaboration](collaboration.md).

## Import, inspect, and publish

**Assets** supports local file selection and drag/drop into an isolated import job. The import sheet shows rights evidence, scale, supported content, estimated resource use, and a preview. A failed or canceled import leaves the project unchanged. Imported house shells appear as decorative models until the creator supplies or approves valid building semantics; the UI exposes this distinction before placement. The creator can inspect exact transforms, hierarchy, dependencies, and changed semantic targets without opening code.

**History** shows accepted revisions separately from unfinished source drafts and running generations. Opening an earlier revision creates a read-only inspection or explicit branch; it cannot move the current shared head. A diff highlights room/wall changes in the world and lists changed source/assets. Recovery offers local draft export when storage, authorization, or sync fails.

**Publish** selects a channel and accepted revision, shows changed content and rights/resource checks, then runs the release protocol in [publishing](publishing.md). The creator sees build/upload progress and can keep editing a later revision. Cancellation does not imply an already committed channel update was undone; the UI resolves its receipt. Success supplies a clean-player link and exact version. Permission or policy requiring review applies to the concrete release, not an unspecified future build. Share controls distinguish private invitations from public links.

## Accessibility and cognitive load

The DOM editor targets WCAG 2.2 AA. Keyboard operation, visible focus, text alternatives, contrast, reflow, and pointer alternatives are explicit acceptance areas. Default touch targets are at least 44 by 44 CSS pixels as a product target. WCAG conformance must be assessed against the whole interaction, not inferred from target size alone. [WCAG 2.2](https://www.w3.org/TR/WCAG22/).

A DOM scene outline and room list provide the same selection and edit operations as the canvas. They expose names, types, relationships, coordinates, and actionable properties. A creator can select **Kitchen → east wall → add doorway** without ray aiming. Semantic names and distances supplement visual highlighting. Screen-reader status announcements summarize user-requested changes and failures; frame-by-frame position updates are opt-in and throttled.

Settings include text size, contrast, remapping, movement speed, hold/toggle controls, reduced motion, head-bob removal, animation pause for editing, and audio controls. Saved preferences are per person. Shared project settings cannot override accessibility preferences. The interface uses short instructions, progressive disclosure, and consistent terms. Success toasts persist long enough to inspect and remain available in history. Critical errors remain until resolved or dismissed.

## Acceptance requirements

- **UX-01:** At 1280 by 720 CSS pixels, the initial view exposes the world and hotbar without a sidebar; opening and closing a tool never changes the world camera.
- **UX-02:** Every core creation-loop action works with keyboard only and through the DOM scene outline; focus returns to the invoking control after closing a panel.
- **UX-03:** Focus loss, pointer-lock refusal, input composition, and cancel clear active movement and prevent unintended commands.
- **UX-04:** Add, resize, move, remove, room extension, doorway editing, behavior attachment, undo, and redo expose clear previews or immediate receipts with exact target scope.
- **UX-05:** An obsolete semantic response cannot change the newly selected object, and its discarded status appears in command history.
- **UX-06:** Touch testing covers placement, navigation, field editing, cancel, undo, and publishing without hidden hover interactions.
- **UX-07:** Reduced-motion and screen-reader modes complete the creation loop; accessibility limitations in player-authored games remain distinct from editor conformance.
