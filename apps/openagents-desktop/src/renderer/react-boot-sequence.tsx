import { type ReactElement } from "react"

import {
  bootSequenceReadyCount,
  bootSequenceScanning,
  type BootSequenceAgentLine,
  type BootSequenceIdentityLine,
} from "./boot-sequence.ts"

const statusGlyph = (status: BootSequenceAgentLine["status"]): string =>
  status === "available" ? "✓" : status === "checking" ? "…" : "✗"

/**
 * Boot Sequence surface (owner directive 2026-07-19): a neutral, terminal-style
 * scan of available agents rendered in the monospace system voice — small and
 * faded, not a chat message. It is pure presentation over the projected agent
 * lines; it holds no discovery authority of its own.
 */
export const ReactBootSequence = ({
  agents,
  identity,
}: {
  readonly agents: ReadonlyArray<BootSequenceAgentLine>
  /** Sovereign identity/wallet lines (IDR-BS #9103). Public identifiers only. */
  readonly identity?: ReadonlyArray<BootSequenceIdentityLine>
}): ReactElement => {
  const ready = bootSequenceReadyCount(agents)
  const scanning = bootSequenceScanning(agents)
  return (
    <section className="oa-react-boot-sequence" aria-label="BOOT SEQUENCE: available agents">
      <ol className="oa-react-boot-lines">
        <li className="oa-react-boot-line" data-kind="title">BOOT SEQUENCE</li>
        <li className="oa-react-boot-line" data-kind="banner">Initializing OpenAgents</li>
        {identity === undefined || identity.length === 0 ? null : (
          <>
            <li className="oa-react-boot-line" data-kind="scan">sovereign identity</li>
            {identity.map((line) => (
              <li key={line.id} className="oa-react-boot-line" data-kind="identity" data-status={line.status}>
                <span aria-hidden="true" className="oa-react-boot-glyph">{statusGlyph(line.status)}</span>
                <span className="oa-react-boot-label">{line.label}</span>
                {line.detail === null ? null : <span className="oa-react-boot-detail">{line.detail}</span>}
                <span className="sr-only">
                  {line.status === "available" ? "available" : line.status === "checking" ? "checking" : "unavailable"}
                </span>
              </li>
            ))}
          </>
        )}
        <li className="oa-react-boot-line" data-kind="scan">scanning for available agents</li>
        {agents.map((agent) => (
          <li key={agent.id} className="oa-react-boot-line" data-kind="agent" data-status={agent.status}>
            <span aria-hidden="true" className="oa-react-boot-glyph">{statusGlyph(agent.status)}</span>
            <span className="oa-react-boot-label">{agent.label}</span>
            {agent.detail === null ? null : <span className="oa-react-boot-detail">{agent.detail}</span>}
            <span className="sr-only">
              {agent.status === "available" ? "available" : agent.status === "checking" ? "checking" : "unavailable"}
            </span>
          </li>
        ))}
        <li aria-live="polite" className="oa-react-boot-line" data-kind="summary" role="status">
          {scanning ? "scanning…" : `${ready} ${ready === 1 ? "agent" : "agents"} ready`}
        </li>
      </ol>
    </section>
  )
}
