import { NodeProps } from "./Node.props"

export const Node = ({ step }: NodeProps) => {
  // Only Step nodes supported for now, so return null if no step
  if (!step) return null

  return (
    <div>
      <p>#{step.order} - {step.name}</p>
      <p>{step.description}</p>
    </div>
  )
}
