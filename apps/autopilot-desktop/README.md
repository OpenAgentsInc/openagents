# Autopilot Desktop

Autopilot Desktop is the Electrobun desktop shell for the Autopilot Coder GUI.
It is planned as a Bun-native sibling to the Pylon TUI: the Bun main process
will own local Pylon control access over loopback, while the webview renders the
operator interface.

The UI is intended to reuse the same Effect and Foldkit stack as
`apps/openagents.com`, with typed bun<->webview RPC as the boundary between
credentialed local control and public-safe projections.

Build, signing, provenance, and update distribution will be defined with the
desktop packaging runbook. Pricing is TBD.
