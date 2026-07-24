# Omega Agent Computer launch surface (AC-02)

- Date: 2026-07-24
- Packet: `OMEGA-AC-02`
- Omega issue: [OpenAgentsInc/omega#29](https://github.com/OpenAgentsInc/omega/issues/29)
- Freeze: [2026-07-24-omega-agent-computer-contract-freeze.md](./2026-07-24-omega-agent-computer-contract-freeze.md)
- Runner receipt: [2026-07-24-omega-agent-computer-runner.md](./2026-07-24-omega-agent-computer-runner.md)

## Result

Omega has an operator-reachable Agent Computer surface that does not wait for
Full Auto UI:

- Dock panel `AgentComputerPanel` in crate `agent_computer_ui`
- Agent menu entry **Agent Computer**
- Actions `agent_computer::OpenPanel` and `agent_computer::StartTurn`
- Start uses framed `run_agent_computer_turn` on supervised `omega-effectd`
- Bearer comes from runtime `OPENAGENTS_AGENT_TOKEN` only
- Panel projects session state, finish reason, and optional artifact ref
- No Omega-only durable cloud thread store

## Verification

- `cargo test -p agent_computer_ui --lib`
- Operator path: Agent menu → Agent Computer → Start cloud turn

## Next

- `OMEGA-AC-03` live Firecracker proof from this surface
