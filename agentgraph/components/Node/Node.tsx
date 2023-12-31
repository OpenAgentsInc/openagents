import { NodeProps } from "./Node.props"
import { NodeContent, NodePanel, NodeTitleBar } from "./Node.styles"

export const Node = ({ step }: NodeProps) => {
  // Only Step nodes supported for now, so return null if no step
  if (!step) return null

  return (
    <NodePanel>
      <NodeTitleBar>
        <p>#{step.order} - {step.name}</p>
      </NodeTitleBar>
      <NodeContent>
        <p>{step.description}</p>
      </NodeContent >
    </NodePanel >
  )
}
