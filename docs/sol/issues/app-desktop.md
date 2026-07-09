# APP-DESKTOP: OpenAgents Desktop — Sarah plus the fleet cockpit on Effect Native

## Outcome

The former Khala Code desktop becomes **OpenAgents Desktop**: Sarah is the
relationship surface and the Fleet pane is the specialist cockpit for deep
coding work.

## Existing substrate

- Shipping Electrobun desktop and Codex/Claude/Grok chat runtimes.
- Dev-only Effect Native fleet cockpit proof.
- Pylon control API, account status, sessions, approvals, and assignment
  execution.
- Shared fleet intents and Khala Sync projections.

## Scope

1. Reframe product name, navigation, and shell as OpenAgents Desktop.
2. Make Sarah conversation and active Blueprint/run state first-class.
3. Build the deep Fleet pane from the same run/work-unit/account/approval
   projections used by `/sarah`.
4. Consume typed Pylon engine/control services; remove stdout parsing and
   duplicate orchestration state only where the replacement is ready.
5. Keep Monaco, terminal, and raw local diagnostics behind typed foreign Host
   nodes as specialist tools.
6. Reauthor the full shell and retained panels on Effect Native; delete legacy
   shell code by converted slice.
7. Preserve signed/notarized distribution and updates-feed contracts under the
   new product name with an explicit release migration plan.

## Non-goals

- Desktop is not a second product home or a separate authority plane.
- Do not block P0 Sarah Fleet Command on completion of the shell conversion.
- Do not retire useful CLI/TUI diagnostics until the replacement is proven.

## Exit

A Sarah-started FleetRun opens in OpenAgents Desktop with matching state and
controls; a desktop-started run is accurately summarized by Sarah. The retained
desktop UI is Effect Native, the product is branded OpenAgents Desktop, and
legacy Khala Code shell paths are deleted.
