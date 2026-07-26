# Omega zero-base mode — 2026-07-26

Design analysis for a bounded Omega mode that shows one Exo thread and hides
the editor around it. This document proposes a surface. It changes no runtime
authority, it allocates no delta number, and it grants nothing. `OMEGA_DELTAS.md`,
owner gate 8, and the Exo boundary in `OMEGA-DELTA-0042` keep their precedence.

Sources: Omega at `origin/main` (`b4e29e1fea`), the `OMEGA_DELTAS.md` entries
0009, 0012, 0020, 0021, 0026, 0031, 0042 and 0046, omega#76, omega#87,
omega#95, and the two Exo documents in
[`docs/exo/`](../exo/2026-07-26-exo-openagents-integration-analysis.md).

## 1. The ask

The owner wants one capability to work first. A demonstration must show Exo,
the controls that operate Exo, and nothing else. The editor around that thread
is not part of the demonstration, and some of it does not work today. Zero base
is therefore a subtraction of the editor. It is not a second product.

One property survives the subtraction, because it costs almost nothing and
because it is the point of an Exo surface. Every thread names the executor that
did its work (`OMEGA-DELTA-0021`). That line is also the control that reaches
Exo. A thread routes to the Exo lane exactly when a person pins `ExternalAcp`
on the pin control that sits on that line. Removal of the line removes the door
into the demonstration.

## 2. What is on screen

The Exo workspace shipped in omega#95 and `OMEGA-DELTA-0046`. Its controls are
in `crates/agent_ui/src/conversation_view/thread_view.rs`:

| Control | Element | Keep in the demonstration |
| --- | --- | --- |
| Workspace header, bolt icon, `Exo workspace` label | `omega-exo-workspace-header` | Yes |
| Executor disclosure line | `executor_disclosure` | Yes, mandatory |
| Executor pin control | `render_executor_pin` | Yes, the way into the lane |
| Turn state dot and label | `exo_status_color` | Yes |
| `Stop` | `omega-exo-cancel-turn` | Yes, present while a turn runs |
| `Inspector` toggle | `omega-exo-toggle-inspector` | Yes |
| Inspector refresh | `omega-exo-refresh-inspection` | Yes, inside the inspector |
| Inspector sections | Identity, Runtime, Capabilities, Authority receipt | Yes, one click away |
| One-turn authorization control | Tier C grant path | Yes, and only when the observed capability needs it |
| Transcript and composer | standard Omega components | Yes |

The finding is short. The Exo surface is already minimal, and nothing inside it
is noise. The noise is the editor that surrounds it. Zero base removes the
surroundings and changes no control in the table.

One judgement call remains inside the surface. The inspector has an expanded
state and a collapsed state. A wide demonstration window should open with the
inspector beside the transcript, because the pin, the commit, the tree, the
binary digest and the writable mounts are the reason a viewer trusts the turn.
A narrow window should open collapsed.

## 3. What is hidden, and by which mechanism

Three mechanisms exist, and each fails differently. The distinction decides
what breaks.

- **Removed.** The code is deleted. `OMEGA-DELTA-0009` and `OMEGA-DELTA-0012`
  removed Restricted Mode and Zed collab this way, because those surfaces never
  return. Zero base must remove nothing. The same binary must still be a full
  editor when the flag is absent.
- **Disabled.** The action still exists and refuses. This is the mechanism that
  keeps a key binding resolvable, which section 5 shows is a startup condition
  and not a preference.
- **Not rendered.** The surface is never built. This is the cheapest mechanism
  and the most dangerous one, because the capability behind the surface stays
  live and a key press still reaches it.

The recommended assignment:

| Surface | Mechanism | Note |
| --- | --- | --- |
| Project, outline, terminal, git and debug panels | Not rendered | Skip the `add_panel_when_ready` calls in `initialize_panels`, in `crates/zed/src/zed.rs` |
| Agent Computer panel, Sarah workroom panel | Not rendered | The same call site |
| Editor pane and tab bar | Not rendered | No project opens, so no buffer exists |
| Command palette entries outside the admitted set | Disabled | `CommandPaletteFilter::hide_namespace`, the mechanism `agent.enabled` already uses in `crates/agent_ui/src/agent_ui.rs` |
| Extensions, feedback and update entries | Disabled | Section 4 shows they reach nothing today |
| Full Auto entry and start control inside the agent panel | Not rendered and disabled | Both, because the capability stays live |
| Any action, any key binding | Never removed | Section 5 |

The rule that makes the table safe: a surface that must not run in zero base is
disabled as well as unrendered. A surface that is only visually absent is still
one key press away.

## 4. What is broken today, and therefore not shown

The owner said that some of the hidden surface does not work. That claim is
correct, and the instances are specific.

- **Extensions.** `auto_install_extensions` is `{}` and `server_url` is
  `https://services.openagents.invalid` (`OMEGA-DELTA-0026`). Omega has no
  extension registry, so the extensions surface reaches nothing.
- **Updates.** `auto_update` is `false` in the same delta, and Omega has no
  update feed. The update entries have no server to ask.
- **Edit predictions.** `edit_predictions.provider` is `"none"` by default, so
  the inline prediction surface does nothing until a person configures a
  provider.
- **Agent Computer.** The panel is registered and on `main`, but the live path
  returned `unauthorized`. The bearer and capacity blocker is open with the
  owner as `AC-03`.
- **Accessibility.** omega#71 closed as a deliberate decision not to build an
  assistive-technology layer. Omega windows expose `AXWindow` with zero
  elements, so no demonstration can be recorded through the accessibility tree.
  Section 7 selects the recorder that this fact permits.

Each of these is one or two clicks from an Exo thread today. That distance is
the practical case for zero base, and it is stronger than any argument about
visual calm.

## 5. Two constraints that bite

**An unresolvable key binding panics Omega before any window opens.** The
built-in keymap is loaded and unwrapped at startup, so a binding that names a
missing action kills the process while `cargo check --workspace` stays green.
`0.2.0-rc6` died this way. `keymaps_name_no_deleted_action` in
`crates/omega_deltas/` exists because of it. Zero base must therefore delete no
action and must not edit `assets/keymaps/default-macos.json` or its Linux and
Windows siblings. Hiding happens in the palette filter and in the action
handler, where a refusal is a sentence a person can read.

**No model-initiated path may start Full Auto authority.** The admitted set of
launch origins is closed at four in `origins_are_all_human_gestures`, and the
admitted set of pin gestures is closed at two in
`pin_gestures_are_all_human_gestures`, both in `crates/omega_front_door/`. A
zero-base mode that pre-pins its thread to the Exo lane needs a third pin
gesture, which means an edit to a closed gate-8 list. The recommendation is to
avoid the edit. Zero base opens the front door with the pin control visible,
and the viewer sets the pin with one click. That click also demonstrates the
disclosure line doing its work, so the cheaper design is also the better
demonstration.

## 6. The flag

Recommendation: **a command-line flag on the shipped binary**, read once at
process start into a process-level mode, and never written to disk.

- A **setting** loses. A settings value is writable by a project settings file
  and by anything else that can write settings, so a mode that hides
  authority-bearing surfaces would be settable by something that is not the
  person at the keyboard. `OMEGA-DELTA-0020` records the same objection against
  a composer mode flag.
- A **release channel** loses. The demonstration becomes a different product
  with its own update path, its own proof matrix, and its own brand review.
- A **separate binary** loses. `OMEGA-DELTA-0038` requires the packaged gate to
  open every executable that ships. A second binary doubles the packaged
  surface that the gate, the delta tests and the notarization path must cover.
- The flag **wins on reversibility**. A person leaves zero base when the
  process ends. Nothing persists, so nothing needs repair.

The mode must also offer a visible way out inside the window, because a viewer
who cannot leave a demonstration will not trust the demonstration.

## 7. The showcase

A viewer sees one window with one surface: the Exo workspace header with the
executor disclosure line, the turn state, the transcript, and the composer. The
viewer types a prompt, watches text deltas and tool calls arrive, presses
`Stop` during a turn, opens the inspector, and reads the source pin, the
measured binary digest, the tool modules, the writable mounts and the durable
Exo session, turn and event references that the completed turn returned.

A viewer who reaches for a removed surface gets a sentence, never a silent
result. The command palette still opens and lists only the admitted actions. An
action that zero base disables answers with one line that names the mode and
the way out of it.

Capture uses the `zed_visual_test_runner` behind `script/omega-visual-proof`,
which draws the real widget tree through Metal inside the test process and
compares each frame against a committed baseline at `MATCH_THRESHOLD` 0.99.
Nothing in that path synthesizes a system key press, so it cannot type into
another application by mistake, and it does not depend on the accessibility
tree that section 4 shows is absent. The Exo suite `run_omega_exo_visual_tests`
already starts the shipped transport, sends one real Exo turn, and records
`omega_exo_workspace_wide` and `omega_exo_workspace_narrow` in
`crates/zed/test_fixtures/visual_tests/`. Zero base adds two baselines beside
them and reuses that suite, because a demonstration that photographs a fake turn
proves nothing.

## 8. Deltas the work needs

Four. This document allocates no number. The highest allocated entry on
`origin/main` is `OMEGA-DELTA-0046`.

1. **Zero base is off unless the process was started with the flag.** The check
   asserts that the shipped defaults contain no zero-base key, and that the mode
   reader names the process command line and not the settings store.
2. **Zero base hides by filter and by refusal, and deletes nothing.** The check
   asserts that the three default keymap files still bind every action zero base
   hides, and that `keymaps_name_no_deleted_action` stays green.
3. **A zero-base turn still names its executor.** The check asserts that the
   typed disclosure record is constructed and drawn on the zero-base surface,
   and that the pin control is present.
4. **Zero base opens no authority path.** The check asserts that
   `LaunchOrigin::all()` and `PinGesture::all()` are unchanged, and that no
   zero-base code path reaches the Full Auto dispatch.

The brand gate needs no fifth entry. `OMEGA-DELTA-0031` derives its prose
inventory from five shipping mechanisms in the source tree and the package, not
from what a window renders, so a hidden surface stays classified. One
consequence is worth a note. If a later change removes code instead of hiding
it, `script/omega-brand-gate.json` holds classifications for literals that no
longer ship, and that file needs a pass at the same time.

## 9. Conflicts, stated rather than resolved

- **The product contract.** `PRODUCT.md` lists *a generic chat panel bolted
  onto an editor* as an anti-reference. Zero base is close to that shape by
  construction. The defence is that the surface keeps the disclosure line, the
  pin, the inspector and the receipts, which is the difference between a chat
  panel and a workspace. The owner should confirm that reading, because the
  contract is the owner's.
- **Identity-first onboarding.** `OMEGA-DELTA-0040` sends a first-ever launch
  to identity onboarding before the front door. A demonstration machine that
  has never run Omega will therefore see onboarding first. The recommendation is
  to keep that order and to record the demonstration on a machine with an
  identity already, rather than to add a bypass. A bypass would be a new way to
  skip an identity gate, which is a larger change than the demonstration needs.
- **The pin gesture.** Section 5 recommends one visible click instead of a third
  gate-8 gesture. If the owner wants the flag itself to pin the lane, that is a
  deliberate edit to a closed list, and it needs its own reason in the test that
  the list carries.
