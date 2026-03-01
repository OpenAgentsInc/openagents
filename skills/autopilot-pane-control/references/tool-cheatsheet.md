# Autopilot Pane Control Cheatsheet

## Pane lifecycle

- `openagents.pane.list {}`
- `openagents.pane.open {"pane":"pane.wallet"}`
- `openagents.pane.focus {"pane":"cad"}`
- `openagents.pane.close {"pane":"autopilot_chat"}`

## Inputs + actions

- `openagents.pane.set_input {"pane":"network_requests","field":"payload","value":"{}"}`
- `openagents.pane.action {"pane":"network_requests","action":"submit_request"}`
- `openagents.pane.action {"pane":"alerts_recovery","action":"select_row","index":0}`
- `openagents.pane.action {"pane":"alerts_recovery","action":"recover_selected"}`

## CAD

- `openagents.cad.intent {"prompt":"Set material al-6061-t6"}`
- `openagents.cad.action {"action":"cycle_variant"}`
- `openagents.cad.action {"action":"select_warning","index":0}`
