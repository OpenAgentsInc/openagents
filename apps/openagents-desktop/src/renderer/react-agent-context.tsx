import { Badge } from "#components/ui/badge"
import { Button } from "#components/ui/button"
import { ComponentValueBinding, IntentRef, type IntentError, type IntentReporter, type JsonPayload } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"
import { ChevronDown, ChevronRight, RefreshCw } from "lucide-react"
import type { ReactElement } from "react"

import type { IdeAgentContextDestination, IdeAgentContextItem } from "../ide/agent-code-contract.ts"
import type { DesktopShellState } from "./shell.ts"

const dispatch = (report: IntentReporter, name: string, payload: JsonPayload = null): void => {
  void Effect.runPromise(report(
    payload === null ? IntentRef(name) : IntentRef(name, ComponentValueBinding()), payload,
  ) as Effect.Effect<void, IntentError>).catch(() => undefined)
}

const destinationLabel = (destination: IdeAgentContextDestination): string => {
  switch (destination._tag) {
    case "HarnessPrompt": return `harness prompt · ${destination.harnessRef}`
    case "ToolInput": return `tool input · ${destination.toolRef}`
    case "LocalMemory": return `local memory · ${destination.policyRef}`
    case "ManagedMemory": return `managed memory · ${destination.placementRef}`
    case "Withheld": return `withheld · ${destination.reason}`
  }
}

const dispositionLabel = (item: IdeAgentContextItem): string => item.disposition._tag === "Included"
  ? `included · ${item.disposition.reason.replaceAll("_", " ")}`
  : `omitted · ${item.disposition.reason.replaceAll("_", " ")} · ${item.disposition.detail}`

const sourceLabel = (item: IdeAgentContextItem): string => item.source._tag === "Unavailable"
  ? item.source.sourceClass.replaceAll("_", " ")
  : item.source._tag

export const AgentContextTray = ({ state, report }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
}): ReactElement => {
  const manifest = state.agentCode.manifests.at(-1) ?? null
  const included = manifest?.items.filter(item => item.disposition._tag === "Included").length ?? 0
  const omitted = manifest?.items.filter(item => item.disposition._tag === "Omitted").length ?? 0
  return <section className="oa-react-agent-context" aria-label="Agent context disclosure">
    {state.agentCodeNotice === null ? null : <p role="alert">{state.agentCodeNotice}</p>}
    <div className="oa-react-agent-context-summary">
      <Button type="button" variant="ghost" size="sm" aria-expanded={state.agentContextTrayOpen}
        aria-controls="agent-context-disclosure" onClick={() => dispatch(report, "DesktopAgentContextTrayToggled")}>
        {state.agentContextTrayOpen ? <ChevronDown aria-hidden="true" /> : <ChevronRight aria-hidden="true" />}
        {manifest === null ? "No agent context manifest" : `Context ${included} included · ${omitted} omitted`}
      </Button>
      <Button type="button" variant="ghost" size="icon-sm" aria-label="Refresh agent context state"
        onClick={() => dispatch(report, "DesktopAgentCodeRefreshed")}><RefreshCw aria-hidden="true" /></Button>
    </div>
    {!state.agentContextTrayOpen ? null : <div id="agent-context-disclosure" className="oa-react-agent-context-detail">
      {manifest === null ? <p>Attach an editor file to assemble an exact, inspectable turn manifest.</p> : <>
        <div className="oa-react-agent-runtime" aria-label="Effective agent runtime">
          <span><b>Harness</b>{manifest.effectiveRuntime.harnessRef}</span>
          <span><b>Model</b>{manifest.effectiveRuntime.modelRef}</span>
          <span><b>Provider / account</b>{manifest.effectiveRuntime.providerRef} · {manifest.effectiveRuntime.accountRef}</span>
          <span><b>Placement</b>{manifest.effectiveRuntime.placementRef} · generation {manifest.effectiveRuntime.placementGeneration}</span>
          <span><b>Permission / sandbox</b>{manifest.effectiveRuntime.permissionMode} · {manifest.effectiveRuntime.sandboxRef}</span>
          <span><b>Retrieval</b>semantic {manifest.effectiveRuntime.semanticRetrieval} · lexical items remain eligible</span>
          <span><b>Retention</b>{manifest.effectiveRuntime.memoryPolicyRef} · {manifest.deletionPolicyRef}</span>
          <span><b>Budget</b>{manifest.includedBytes.toLocaleString()} / {manifest.byteBudget.toLocaleString()} bytes · {manifest.includedTokens.toLocaleString()} / {manifest.tokenBudget.toLocaleString()} tokens</span>
        </div>
        <ol className="oa-react-agent-context-items">
          {manifest.items.map(item => <li key={item.contextItemRef} data-disposition={item.disposition._tag.toLowerCase()}>
            <div><Badge variant={item.disposition._tag === "Included" ? "secondary" : "outline"}>{item.disposition._tag}</Badge><strong>{item.label}</strong><small>{sourceLabel(item)} · generation {item.source.sourceGeneration}</small></div>
            <p>{dispositionLabel(item)}</p>
            <dl>
              <div><dt>Destination</dt><dd>{destinationLabel(item.destination)}</dd></div>
              <div><dt>Size</dt><dd>{item.byteEstimate.toLocaleString()} bytes · {item.tokenEstimate.toLocaleString()} tokens{item.truncated ? " · truncated" : ""}</dd></div>
              <div><dt>Handling</dt><dd>{item.sensitivity} · {item.freshness} · {item.retention}</dd></div>
            </dl>
          </li>)}
        </ol>
      </>}
    </div>}
  </section>
}
